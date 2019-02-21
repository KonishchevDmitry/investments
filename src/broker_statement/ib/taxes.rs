use crate::broker_statement::taxes::{TaxId, TaxChanges};
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date};

pub struct WithholdingTaxParser {}

impl RecordParser for WithholdingTaxParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency == "Total" {
            return Ok(());
        }

        let date = parse_date(record.get_value("Date")?)?;
        let description = record.get_value("Description")?;
        let tax_id = TaxId::new(date, description);

        // Tax amount is represented as a negative number.
        //
        // Positive number is used to cancel a previous tax payment and usually followed by another
        // negative number.
        let tax = Cash::new(currency, record.parse_cash("Amount", DecimalRestrictions::NonZero)?);

        let tax_changes = parser.statement.tax_changes.entry(tax_id)
            .or_insert_with(TaxChanges::new);

        if tax.is_positive() {
            tax_changes.refund(tax);
        } else {
            tax_changes.withhold(-tax);
        }

        Ok(())
    }
}