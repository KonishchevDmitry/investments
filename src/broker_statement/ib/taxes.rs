use broker_statement::ib::IbStatementParser;
use broker_statement::ib::common::{Record, RecordParser, parse_date};
use core::EmptyResult;
use currency::Cash;
use types::Date;
use util;

pub type TaxId = (Date, String);

pub struct WithholdingTaxParser {}

impl RecordParser for WithholdingTaxParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency == "Total" {
            return Ok(());
        }

        let date = parse_date(record.get_value("Date")?)?;
        let description = record.get_value("Description")?.to_owned();

        let tax_id = (date, description.clone());
        let mut tax = Cash::new_from_string(currency, record.get_value("Amount")?)?;

        // Tax amount is represented as a negative number.
        // Positive number is used to cancel a previous tax payment and usually followed by another
        // negative number.
        if tax.is_zero() {
            return Err!("Invalid withholding tax: {}", tax.amount);
        } else if tax.is_positive() {
            return match parser.taxes.remove(&tax_id) {
                Some(cancelled_tax) if cancelled_tax == tax => Ok(()),
                _ => Err!("Invalid withholding tax: {}", tax.amount),
            }
        }

        tax = -tax;

        if let Some(_) = parser.taxes.insert(tax_id, tax) {
            return Err!("Got a duplicate withholding tax: {} / {:?}",
                util::format_date(date), description);
        }

        Ok(())
    }
}
