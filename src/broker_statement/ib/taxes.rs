use lazy_static::lazy_static;
use regex::Regex;

use crate::broker_statement::taxes::{TaxId, TaxAccruals};
use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date};

pub struct WithholdingTaxParser {}

impl RecordParser for WithholdingTaxParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        let date = parse_date(record.get_value("Date")?)?;
        let issuer = parse_tax_description(record.get_value("Description")?)?;
        let tax_id = TaxId::new(date, &issuer);

        // Tax amount is represented as a negative number.
        //
        // Positive number is used to cancel a previous tax payment and usually followed by another
        // negative number.
        let tax = Cash::new(currency, record.parse_cash("Amount", DecimalRestrictions::NonZero)?);

        let accruals = parser.statement.tax_accruals.entry(tax_id)
            .or_insert_with(TaxAccruals::new);

        if tax.is_positive() {
            accruals.reverse(tax);
        } else {
            accruals.add(-tax);
        }

        Ok(())
    }
}

fn parse_tax_description(description: &str) -> GenericResult<String> {
    lazy_static! {
        static ref DESCRIPTION_REGEX: Regex = Regex::new(
            r"^(?P<issuer>[A-Z]+) ?\([A-Z0-9]+\) Cash Dividend .+? - US Tax$").unwrap();
    }

    let captures = DESCRIPTION_REGEX.captures(description).ok_or_else(|| format!(
        "Unexpected tax description: {:?}", description))?;

    Ok(captures.name("issuer").unwrap().as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tax_parsing() {
        test_tax_parsing("BND (US9219378356) Cash Dividend USD 0.181007 - US Tax", "BND");
        test_tax_parsing("BND(US9219378356) Cash Dividend USD 0.193413 per Share - US Tax", "BND");
        test_tax_parsing("BND(US9219378356) Cash Dividend 0.18366600 USD per Share - US Tax", "BND");
        test_tax_parsing("BND(43645828) Cash Dividend 0.19446400 USD per Share - US Tax", "BND");
    }

    fn test_tax_parsing(description: &str, symbol: &str) {
        assert_eq!(parse_tax_description(description).unwrap(), symbol.to_owned());
    }
}
