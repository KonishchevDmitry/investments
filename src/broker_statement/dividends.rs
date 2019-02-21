use std::collections::HashMap;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;
use crate::types::{Date, Decimal};

use super::TaxId;

#[derive(Debug)]
pub struct Dividend {
    pub date: Date,
    pub issuer: String,
    pub amount: Cash,
    pub paid_tax: Cash,
}

impl Dividend {
    pub fn tax_to_pay(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Decimal> {
        let amount = converter.convert_to(self.date, self.amount, country.currency)?;
        let paid_tax = converter.convert_to(self.date, self.paid_tax, country.currency)?;
        Ok(country.tax_to_pay(amount, Some(paid_tax)))
    }
}

pub struct DividendWithoutPaidTax {
    pub date: Date,
    pub issuer: String,
    pub amount: Cash,
    pub tax_extractor: Box<DividendPaidTaxExtractor>,
}

impl DividendWithoutPaidTax {
    pub fn upgrade(self, taxes: &mut HashMap<TaxId, Cash>) -> GenericResult<Dividend> {
        let paid_tax = self.tax_extractor.get_paid_tax(taxes).map_err(|e| format!(
            "Unable to match {} dividend from {} to paid taxes: {}",
            self.issuer, formatting::format_date(self.date), e))?;

        Ok(Dividend {
            date: self.date,
            issuer: self.issuer,
            amount: self.amount,
            paid_tax: paid_tax,
        })
    }
}

pub trait DividendPaidTaxExtractor {
    fn get_paid_tax(&self, taxes: &mut HashMap<TaxId, Cash>) -> GenericResult<Cash>;
}

pub struct TaxIdExtractor {
    tax_id: TaxId,
}

impl TaxIdExtractor {
    pub fn new(tax_id: TaxId) -> Box<DividendPaidTaxExtractor> {
        Box::new(TaxIdExtractor {tax_id: tax_id})
    }
}

impl DividendPaidTaxExtractor for TaxIdExtractor {
    fn get_paid_tax(&self, taxes: &mut HashMap<TaxId, Cash>) -> GenericResult<Cash> {
        Ok(taxes.remove(&self.tax_id).ok_or_else(|| format!(
            "There is no tax with {:?} expected description", self.tax_id.description
        ))?)
    }
}