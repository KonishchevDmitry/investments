use lazy_static::lazy_static;
use regex::Regex;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::broker_statement::dividends::{DividendId, DividendAccruals};
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date};

pub struct DividendsParser {}

impl RecordParser for DividendsParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        let date = parse_date(record.get_value("Date")?)?;
        let issuer = parse_dividend_description(record.get_value("Description")?)?;
        let amount = Cash::new(currency, record.parse_cash(
            "Amount", DecimalRestrictions::NonZero)?);

        let accruals = parser.statement.dividend_accruals.entry(DividendId {
            date: date,
            issuer: issuer,
        }).or_insert_with(DividendAccruals::new);

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
            r"^(?P<issuer>[A-Z]+) ?\([A-Z0-9]+\) Cash Dividend ").unwrap();
    }

    let captures = DESCRIPTION_REGEX.captures(description).ok_or_else(|| format!(
        "Unexpected dividend description: {:?}", description))?;

    Ok(captures.name("issuer").unwrap().as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dividend_parsing() {
        test_parsing("VNQ (US9229085538) Cash Dividend USD 0.7318 (Ordinary Dividend)", "VNQ");
        test_parsing("IEMG(US46434G1031) Cash Dividend 0.44190500 USD per Share (Ordinary Dividend)", "IEMG");

        test_parsing("BND(US9219378356) Cash Dividend 0.18685800 USD per Share (Mixed Income)", "BND");
        test_parsing("VNQ(US9229085538) Cash Dividend 0.82740000 USD per Share (Return of Capital)", "VNQ");

        test_parsing("BND(US9219378356) Cash Dividend USD 0.193413 per Share (Ordinary Dividend)", "BND");
        test_parsing("BND(US9219378356) Cash Dividend USD 0.193413 per Share - Reversal (Ordinary Dividend)", "BND");
    }

    fn test_parsing(description: &str, symbol: &str) {
        assert_eq!(parse_dividend_description(description).unwrap(), symbol.to_owned());
    }
}