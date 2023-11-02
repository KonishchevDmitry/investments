use std::iter::Iterator;
use std::str::FromStr;

use csv::StringRecord;
use cusip::CUSIP;
use isin::ISIN;
use lazy_static::lazy_static;
use regex::Regex;

use crate::broker_statement::ib::StatementParser;
use crate::core::{EmptyResult, GenericResult, GenericError};
use crate::currency::Cash;
use crate::time;
use crate::types::{Date, DateTime, Decimal};
use crate::util::{self, DecimalRestrictions};

pub const STOCK_SYMBOL_REGEX: &str = r"(?:[A-Z][A-Z0-9]*[a-z]*|\d+)(?:[ .][A-Z]+)??";
pub const OLD_SYMBOL_SUFFIX: &str = ".OLD";

// IB uses the following identifier types as security ID:
// * ISIN (it seems that IB uses only this type in broker statements since 2020)
// * CUSIP - US standard which in most cases may be converted to ISIN, but not always (see
//   https://stackoverflow.com/questions/30545239/convert-9-digit-cusip-codes-into-isin-codes)
// * conid (contract ID) - an internal IB's instrument UID
#[derive(Debug)]
pub enum SecurityID {
    Isin(ISIN),
    Cusip(CUSIP),
    Conid(u32),
}

impl SecurityID {
    pub const REGEX: &'static str = "[A-Z0-9]+";
}

impl FromStr for SecurityID {
    type Err = GenericError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(if let Ok(isin) = value.parse() {
            SecurityID::Isin(isin)
        } else if let Ok(cusip) = value.parse() {
            SecurityID::Cusip(cusip)
        } else if let Ok(conid) = value.parse() {
            SecurityID::Conid(conid)
        } else {
            return Err!("Unsupported security ID: {:?}", value);
        })
    }
}

pub struct RecordSpec<'a> {
    pub name: &'a str,
    fields: Vec<&'a str>,
    offset: usize,
}

impl<'a> RecordSpec<'a> {
    pub fn new(name: &'a str, fields: Vec<&'a str>, offset: usize) -> RecordSpec<'a> {
        RecordSpec {name, fields, offset}
    }

    pub fn has_field(&self, field: &str) -> bool {
        self.field_index(field).is_some()
    }

    fn field_index(&self, field: &str) -> Option<usize> {
        self.fields.iter().position(|other: &&str| *other == field)
    }
}

pub struct Record<'a> {
    pub spec: &'a RecordSpec<'a>,
    pub values: &'a StringRecord,
}

impl<'a> Record<'a> {
    pub fn new(spec: &'a RecordSpec<'a>, values: &'a StringRecord) -> Record<'a> {
        Record {spec, values}
    }

    pub fn get_value(&self, field: &str) -> GenericResult<&str> {
        if let Some(index) = self.spec.field_index(field) {
            if let Some(value) = self.values.get(self.spec.offset + index) {
                return Ok(value);
            }
        }

        Err!("{:?} record doesn't have {:?} field", self.spec.name, field)
    }

    pub fn check_value(&self, field: &str, value: &str) -> EmptyResult {
        self.check_values(&[(field, value)])
    }

    pub fn check_values(&self, values: &[(&str, &str)]) -> EmptyResult {
        for (field, expected_value) in values {
            let value = self.get_value(field)?;
            if value != *expected_value {
                return Err!("Got an unexpected {:?} field value: {:?}", field, value);
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn parse_value<T: FromStr>(&self, field: &str) -> GenericResult<T> {
        let value = self.get_value(field)?;
        Ok(value.parse().map_err(|_| format!(
            "{:?} field has an invalid value: {:?}", field, value))?)
    }

    pub fn parse_date(&self, field: &str) -> GenericResult<Date> {
        parse_date(self.get_value(field)?)
    }

    pub fn parse_date_time(&self, field: &str) -> GenericResult<DateTime> {
        parse_date_time(self.get_value(field)?)
    }

    pub fn parse_symbol(&self, field: &str) -> GenericResult<String> {
        parse_symbol(self.get_value(field)?)
    }

    pub fn parse_quantity(&self, field: &str, restrictions: DecimalRestrictions) -> GenericResult<Decimal> {
        let quantity = parse_quantity(self.get_value(field)?)?;
        util::validate_named_decimal("quantity", quantity, restrictions)
    }

    pub fn parse_amount(&self, field: &str, restrictions: DecimalRestrictions) -> GenericResult<Decimal> {
        let value = self.get_value(field)?;
        let amount = parse_quantity(value).map_err(|_| format!("Invalid amount: {:?}", value))?;
        util::validate_named_decimal("amount", amount, restrictions)
    }

    pub fn parse_cash(&self, field: &str, currency: &str, restrictions: DecimalRestrictions) -> GenericResult<Cash> {
        Ok(Cash::new(currency, self.parse_amount(field, restrictions)?))
    }
}

pub trait RecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> { Some(&["Data"]) }
    fn skip_data_types(&self) -> Option<&'static [&'static str]> { None }
    fn skip_totals(&self) -> bool { false }
    fn allow_multiple(&self) -> bool { false }
    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult;
}

pub struct UnknownRecordParser {}

impl RecordParser for UnknownRecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> {
        None
    }

    fn allow_multiple(&self) -> bool {
        true
    }

    fn parse(&mut self, _parser: &mut StatementParser, _record: &Record) -> EmptyResult {
        Ok(())
    }
}

pub fn check_volume(quantity: Decimal, price: Cash, volume: Cash) -> EmptyResult {
    if !cfg!(debug_assertions) {
        return Ok(());
    }

    let expected_volume = price * quantity;

    for precision in 2..=8 {
        if expected_volume.round_to(precision) == volume {
            return Ok(());
        }
    }

    Err!("Got an unexpected volume: {} vs {}", volume, expected_volume)
}

pub fn is_header_field(value: &str) -> bool {
    matches!(value, "Header" | "Headers") // https://github.com/KonishchevDmitry/investments/issues/81
}

pub fn format_record<'a, I>(iter: I) -> String
    where I: IntoIterator<Item = &'a str> {

    iter.into_iter()
        .map(|value| format!("{:?}", value))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn format_error_record(record: &StringRecord) -> String {
    let mut human = format!("({})", format_record(record));

    if let Some(position) = record.position() {
        human = format!("{} ({} line)", human, position.line());
    }

    human
}

pub fn parse_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%Y-%m-%d")
}

pub fn parse_date_time(date_time: &str) -> GenericResult<DateTime> {
    time::parse_date_time(date_time, "%Y-%m-%d, %H:%M:%S").or_else(|_|
        time::parse_date_time(date_time, "%Y-%m-%d %H:%M:%S"))
}

pub fn parse_symbol(symbol: &str) -> GenericResult<String> {
    lazy_static! {
        static ref SYMBOL_REGEX: Regex = Regex::new(&format!(
            r"^{}$", STOCK_SYMBOL_REGEX)).unwrap();
    }

    if !SYMBOL_REGEX.is_match(symbol) || symbol.ends_with(OLD_SYMBOL_SUFFIX) {
        return Err!("Got a stock symbol with an unsupported format: {:?}", symbol);
    }

    Ok(symbol.replace(' ', "-").to_uppercase())
}

fn parse_quantity(quantity: &str) -> GenericResult<Decimal> {
    // See https://github.com/KonishchevDmitry/investments/issues/34

    lazy_static! {
        static ref DECIMAL_SEPARATOR_REGEX: Regex = Regex::new(
            r"([1-9]0*),(\d{3})(,|\.|$)").unwrap();
    }
    let mut stripped = quantity.to_owned();

    while stripped.contains(',') {
        let new = DECIMAL_SEPARATOR_REGEX.replace_all(&stripped, "$1$2$3");
        if new == stripped {
            return Err!("Invalid quantity: {:?}", quantity)
        }
        stripped = new.into_owned();
    }

    Ok(Decimal::from_str(&stripped).map_err(|_| format!("Invalid quantity: {:?}", quantity))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    use matches::assert_matches;
    use rstest::rstest;

    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("2018-06-22").unwrap(), date!(2018, 6, 22));
    }

    #[rstest(value,
        case("2018-07-31 13:09:47"),
        case("2018-07-31, 13:09:47"),
    )]
    fn time_parsing(value: &str) {
        assert_eq!(parse_date_time(value).unwrap(), date_time!(2018, 7, 31, 13, 9, 47));
    }

    #[rstest(value, expected,
        case("1020", Some(dec!(1020))),
        case("1,020", Some(dec!(1020))),
        case("1,020,304.05", Some(dec!(1_020_304.05))),
        case("-1,020,304.05", Some(dec!(-1_020_304.05))),
        case("1,000,000.05", Some(dec!(1_000_000.05))),
        case("-1,000,000.05", Some(dec!(-1_000_000.05))),

        case(",102", None),
        case("102,", None),
        case("0,102", None),
        case("10,20", None),
        case("10,20.3", None),
        case("1,0203", None),
    )]
    fn quantity_parsing(value: &str, expected: Option<Decimal>) {
        if let Some(expected) = expected {
            assert_eq!(parse_quantity(value).unwrap(), expected);
        } else {
            assert_eq!(
                parse_quantity(value).unwrap_err().to_string(),
                format!("Invalid quantity: {:?}", value),
            );
        }
    }

    #[test]
    fn security_id_parsing() {
        let parse = |s| SecurityID::from_str(s).unwrap();

        // BND
        assert_matches!(parse("US9219378356"), SecurityID::Isin(_));
        assert_matches!(parse("921937835"), SecurityID::Cusip(_));
        assert_matches!(parse("43645828"), SecurityID::Conid(conid) if conid == 43645828);
    }

    #[rstest(value, expected,
        case("T",       "T"),
        case("VTI",     "VTI"),
        case("TKAd",    "TKAD"),    // https://github.com/KonishchevDmitry/investments/issues/64
        case("1086",    "1086"),    // https://github.com/KonishchevDmitry/investments/issues/64
        case("U.UN",    "U.UN"),    // https://github.com/KonishchevDmitry/investments/issues/62
        case("RDS B",   "RDS-B"),   // https://github.com/KonishchevDmitry/investments/issues/28
        case("CBL PRD", "CBL-PRD"), // https://github.com/KonishchevDmitry/investments/issues/42
    )]
    fn symbol_parsing(value: &str, expected: &str) {
        assert_eq!(parse_symbol(value).unwrap(), expected);
    }
}