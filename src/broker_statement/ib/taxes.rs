use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::formatting;
use crate::types::Date;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date};

pub type TaxId = (Date, String);

pub struct WithholdingTaxParser {}

impl RecordParser for WithholdingTaxParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency == "Total" {
            return Ok(());
        }

        let date = parse_date(record.get_value("Date")?)?;
        let description = record.get_value("Description")?.to_owned();

        let tax_id = (date, description.clone());
        let mut tax = Cash::new(
            currency, record.parse_cash("Amount", DecimalRestrictions::NonZero)?);

        // Tax amount is represented as a negative number.
        // Positive number is used to cancel a previous tax payment and usually followed by another
        // negative number.
        if tax.is_positive() {
            return match parser.taxes.remove(&tax_id) {
                Some(cancelled_tax) if cancelled_tax == tax => Ok(()),
                _ => Err!("Invalid withholding tax: {}", tax.amount),
            }
        }

        tax = -tax;

        if parser.taxes.insert(tax_id, tax).is_some() {
            return Err!("Got a duplicate withholding tax: {} / {:?}",
                formatting::format_date(date), description);
        }

        Ok(())
    }
}
