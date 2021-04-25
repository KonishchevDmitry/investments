use std::iter::Iterator;
use std::str::FromStr;

use csv::StringRecord;
use lazy_static::lazy_static;
use regex::Regex;

use crate::broker_statement::ib::StatementParser;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::time;
use crate::types::{Date, DateTime, Decimal};
use crate::util::{self, DecimalRestrictions};

pub const STOCK_ID_REGEX: &str = "[A-Z0-9]+?";
pub const STOCK_SYMBOL_REGEX: &str = "[A-Z][A-Z0-9]*?(:? [A-Z0-9]+)?";

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
        for (field, value) in values.iter() {
            if self.get_value(*field)? != *value {
                return Err!("Got an unexpected {:?} field value: {:?}", *field, *value);
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

pub fn format_record<'a, I>(iter: I) -> String
    where I: IntoIterator<Item = &'a str> {

    iter.into_iter()
        .map(|value| format!("{:?}", value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%Y-%m-%d")
}

pub fn parse_date_time(date_time: &str) -> GenericResult<DateTime> {
    time::parse_date_time(date_time, "%Y-%m-%d, %H:%M:%S")
}

pub fn parse_symbol(symbol: &str) -> GenericResult<String> {
    lazy_static! {
        static ref SYMBOL_REGEX: Regex = Regex::new(&format!(
            r"^{}$", STOCK_SYMBOL_REGEX)).unwrap();
    }

    if !SYMBOL_REGEX.is_match(symbol) {
        return Err!("Got a stock symbol with an unsupported format: {:?}", symbol);
    }

    // See https://github.com/KonishchevDmitry/investments/issues/28
    Ok(symbol.replace(' ', "-"))
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
    use rstest::rstest;

    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("2018-06-22").unwrap(), date!(22, 6, 2018));
    }

    #[test]
    fn time_parsing() {
        assert_eq!(parse_date_time("2018-07-31, 13:09:47").unwrap(), date!(31, 7, 2018).and_hms(13, 9, 47));
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
}