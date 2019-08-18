use lazy_static::lazy_static;
use regex::Regex;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::broker_statement::taxes::TaxId;
use crate::broker_statement::dividends::{DividendId, DividendAccruals};
use crate::formatting;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date};

pub struct DividendsParser {}

impl RecordParser for DividendsParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency == "Total" {
            return Ok(());
        }

        let date = parse_date(record.get_value("Date")?)?;
        let description = record.get_value("Description")?;
        let (issuer, short_description, taxable, reversal) =
            parse_dividend_description(description)?;
        let amount = Cash::new(currency, record.parse_cash(
            "Amount", DecimalRestrictions::NonZero)?);

        if amount.is_negative() != reversal {
            return Err!("{} dividend from {}: Got an unexpected amount: {}",
                        issuer, formatting::format_date(date), amount);
        }

        parser.statement.dividend_accruals.entry(DividendId {
            date: date,
            issuer: issuer,
            description: short_description.clone(),
            tax_description: if taxable {
                Some(short_description + " - US Tax")
            } else {
                None
            },
        }).and_modify(|accruals| {
            if reversal {
                accruals.reverse(-amount)
            } else {
                accruals.add(amount)
            }
        }).or_insert_with(DividendAccruals::new);

        Ok(())
    }
}

fn parse_dividend_description(description: &str) -> GenericResult<(String, String, bool, bool)> {
    lazy_static! {
        static ref DESCRIPTION_REGEX: Regex = Regex::new(r"^(?x)
            (?P<description>
                (?P<issuer>[A-Z]+)\b
                .+?
            )
            (?P<reversal>(?-x) - Reversal)?
            \s\(
                (?P<type>[^)]+)
            \)
            $
        ").unwrap();
    }

    let captures = DESCRIPTION_REGEX.captures(description).ok_or_else(|| format!(
        "Unexpected dividend description: {:?}", description))?;

    let issuer = captures.name("issuer").unwrap().as_str().to_owned();
    let income_type = captures.name("type").unwrap().as_str().to_owned();
    let short_description = captures.name("description").unwrap().as_str().to_owned();

    let taxable = income_type != "Return of Capital";
    let reversal = captures.name("reversal").is_some();

    Ok((issuer, short_description, taxable, reversal))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dividend_parsing() {
        test_dividend_parsing(
            "VNQ (US9229085538) Cash Dividend USD 0.7318 (Ordinary Dividend)",
            "VNQ", "VNQ (US9229085538) Cash Dividend USD 0.7318", true, false,
        );

        test_dividend_parsing(
            "IEMG(US46434G1031) Cash Dividend 0.44190500 USD per Share (Ordinary Dividend)",
            "IEMG", "IEMG(US46434G1031) Cash Dividend 0.44190500 USD per Share", true, false,
        );

        test_dividend_parsing(
            "BND(US9219378356) Cash Dividend 0.18685800 USD per Share (Mixed Income)",
            "BND", "BND(US9219378356) Cash Dividend 0.18685800 USD per Share", true, false,
        );
    }

    #[test]
    fn non_taxable_dividend_parsing() {
        test_dividend_parsing(
            "VNQ(US9229085538) Cash Dividend 0.82740000 USD per Share (Return of Capital)",
            "VNQ", "VNQ(US9229085538) Cash Dividend 0.82740000 USD per Share", false, false,
        );
    }

    #[test]
    fn dividend_reversal_parsing() {
        test_dividend_parsing(
            "BND(US9219378356) Cash Dividend USD 0.193413 per Share (Ordinary Dividend)",
            "BND", "BND(US9219378356) Cash Dividend USD 0.193413 per Share", true, false,
        );

        test_dividend_parsing(
            "BND(US9219378356) Cash Dividend USD 0.193413 per Share - Reversal (Ordinary Dividend)",
            "BND", "BND(US9219378356) Cash Dividend USD 0.193413 per Share", true, true,
        );
    }

    fn test_dividend_parsing(input: &str, symbol: &str, description: &str, taxable: bool, reversal: bool) {
        assert_eq!(
            parse_dividend_description(input).unwrap(),
            (symbol.to_owned(), description.to_owned(), taxable, reversal),
        );
    }
}