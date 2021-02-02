use lazy_static::lazy_static;
use regex::Regex;

use crate::core::{EmptyResult, GenericResult};
use crate::broker_statement::dividends::DividendId;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser};

pub struct DividendsParser {}

impl RecordParser for DividendsParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        let date = record.parse_date("Date")?;
        let issuer = parse_dividend_description(record.get_value("Description")?)?;
        let amount = record.parse_cash("Amount", currency, DecimalRestrictions::NonZero)?;

        let dividend_id = DividendId::new(date, &issuer);
        let accruals = parser.statement.dividend_accruals.entry(dividend_id).or_default();

        if amount.is_negative() {
            accruals.reverse(-amount)
        } else {
            accruals.add(amount)
        }

        Ok(())
    }
}

fn parse_dividend_description(description: &str) -> GenericResult<String> {
    lazy_static! {
        static ref DESCRIPTION_REGEX: Regex = Regex::new(
            r"^(?P<issuer>[A-Z]+) ?\([A-Z0-9]+\) ").unwrap();
    }

    let captures = DESCRIPTION_REGEX.captures(description).ok_or_else(|| format!(
        "Unexpected dividend description: {:?}", description))?;

    Ok(captures.name("issuer").unwrap().as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(description, symbol,
        case("VNQ (US9229085538) Cash Dividend USD 0.7318 (Ordinary Dividend)", "VNQ"),
        case("IEMG(US46434G1031) Cash Dividend 0.44190500 USD per Share (Ordinary Dividend)", "IEMG"),

        case("BND(US9219378356) Cash Dividend 0.18685800 USD per Share (Mixed Income)", "BND"),
        case("VNQ(US9229085538) Cash Dividend 0.82740000 USD per Share (Return of Capital)", "VNQ"),

        case("BND(US9219378356) Cash Dividend USD 0.193413 per Share (Ordinary Dividend)", "BND"),
        case("BND(US9219378356) Cash Dividend USD 0.193413 per Share - Reversal (Ordinary Dividend)", "BND"),

        case("UNIT(US91325V1089) Payment in Lieu of Dividend (Ordinary Dividend)", "UNIT"),
    )]
    fn dividend_parsing(description: &str, symbol: &str) {
        assert_eq!(parse_dividend_description(description).unwrap(), symbol.to_owned());
    }
}