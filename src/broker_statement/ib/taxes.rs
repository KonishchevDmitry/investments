use std::collections::HashMap;

use crate::broker_statement::taxes::TaxChanges;
use crate::core::{EmptyResult, GenericResult};
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

        // Tax amount is represented as a negative number.
        //
        // Positive number is used to cancel a previous tax payment and usually followed by another
        // negative number.
        let tax = Cash::new(currency, record.parse_cash("Amount", DecimalRestrictions::NonZero)?);
        let tax_changes = parser.tax_changes.entry(tax_id).or_insert_with(TaxChanges::new);

        if tax.is_positive() {
            tax_changes.refund(tax);
        } else {
            tax_changes.withhold(-tax);
        }

        Ok(())
    }
}

pub fn parse_taxes(mut tax_changes: HashMap<TaxId, TaxChanges>) -> GenericResult<HashMap<TaxId, Cash>> {
    let mut taxes = HashMap::new();

    for (id, changes) in tax_changes.drain() {
        let date = id.0;
        let description = id.1.clone();

        let tax = changes.get_result_tax().map_err(|e| format!(
            "Failed to process {} / {:?} tax: {}",
            formatting::format_date(date), description, e))?;

        assert!(taxes.insert(id, tax).is_none());
    }

    Ok(taxes)
}