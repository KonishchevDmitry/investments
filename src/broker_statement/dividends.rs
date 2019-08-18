use std::collections::HashMap;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;
use crate::types::{Date, Decimal};

use super::TaxId;
use super::payments::Payments;

#[derive(Debug)]
pub struct Dividend {
    pub date: Date,
    pub issuer: String,
    pub amount: Cash,
    pub paid_tax: Cash,
}

impl Dividend {
    pub fn tax(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Decimal> {
        let amount = converter.convert_to(self.date, self.amount, country.currency)?;
        Ok(country.tax_to_pay(amount, None))
    }

    pub fn tax_to_pay(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Decimal> {
        let amount = converter.convert_to(self.date, self.amount, country.currency)?;
        let paid_tax = converter.convert_to(self.date, self.paid_tax, country.currency)?;
        Ok(country.tax_to_pay(amount, Some(paid_tax)))
    }

    pub fn description(&self) -> String {
        format!("{} dividend from {}", self.issuer, formatting::format_date(self.date))
    }
}

pub struct DividendWithoutPaidTax {
    pub date: Date,
    pub issuer: String,
    pub amount: Cash,
    pub tax_extractor: Box<dyn DividendPaidTaxExtractor>,
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

#[derive(PartialEq, Eq, Hash)]
pub struct DividendId {
    pub date: Date,
    pub issuer: String,
    pub description: String,
    pub tax_description: Option<String>,
}

pub fn process_dividends(dividend: DividendId, changes: Payments, taxes: &mut HashMap<TaxId, Payments>) -> GenericResult<Option<Dividend>> {
    let paid_tax = dividend.tax_description.as_ref().map_or(Ok(None), |tax_description| {
        let tax_id = TaxId::new(dividend.date, &tax_description);
        let tax_changes = taxes.remove(&tax_id).ok_or_else(|| format!(
            "There is no tax with {:?} expected description", tax_description
        ))?;

        tax_changes.get_result().map_err(|e| format!(
            "Failed to process {} / {:?} tax: {}",
            formatting::format_date(dividend.date), tax_description, e))
    }).map_err(|e| format!(
        "Unable to match {} dividend from {} to paid taxes: {}",
        dividend.issuer, formatting::format_date(dividend.date), e)
    )?;

    let amount = changes.get_result().map_err(|e| format!(
        "Failed to process {} dividend from {}: {}",
        dividend.issuer, formatting::format_date(dividend.date), e))?;

    Ok(amount.map(|amount| {
        Dividend {
            date: dividend.date,
            issuer: dividend.issuer,
            amount: amount,
            paid_tax: paid_tax.unwrap_or_else(|| Cash::new(amount.currency, dec!(0))),
        }
    }))
}

pub trait DividendPaidTaxExtractor {
    fn get_paid_tax(&self, taxes: &mut HashMap<TaxId, Cash>) -> GenericResult<Cash>;
}

pub struct TaxIdExtractor {
    tax_id: TaxId,
}

impl TaxIdExtractor {
    pub fn new(tax_id: TaxId) -> Box<dyn DividendPaidTaxExtractor> {
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