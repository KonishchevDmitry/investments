use std::collections::HashMap;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::formatting;
use crate::types::Date;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date};

pub type TaxId = (Date, String);

pub struct TaxChanges {
    withheld: Vec<Cash>,
    refunded: Vec<Cash>,
}

impl TaxChanges {
    fn new() -> TaxChanges {
        TaxChanges {
            withheld: Vec::new(),
            refunded: Vec::new(),
        }
    }

    fn get_result_tax(self) -> GenericResult<Cash> {
        let TaxChanges { mut withheld, refunded } = self;

        for refund in refunded {
            let index = withheld.iter()
                .position(|&amount| amount == refund)
                .ok_or_else(|| format!(
                    "Unexpected tax refund: {}. Unable to find the matching withheld tax", refund))?;

            withheld.remove(index);
        }

        match withheld.len() {
            // It's may be ok, but for now return an error until we'll see it in the real life
            0 => Err!("Got a fully refunded tax"),

            1 => Ok(withheld.pop().unwrap()),
            _ => Err!("Got {} withheld taxes without refund", withheld.len()),
        }
    }
}

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
        // negative number. But in practice the situation when refund goes before the matched
        // withheld tax is very regular.
        let tax = Cash::new(currency, record.parse_cash("Amount", DecimalRestrictions::NonZero)?);
        let tax_changes = parser.tax_changes.entry(tax_id).or_insert_with(TaxChanges::new);

        if tax.is_positive() {
            tax_changes.refunded.push(tax);
        } else {
            tax_changes.withheld.push(-tax);
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