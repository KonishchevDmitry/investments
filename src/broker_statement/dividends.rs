use std::collections::HashMap;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
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
    date: Date,
    issuer: String,
    amount: Cash,
    tax_extractor: Box<DividendPaidTaxExtractor>,
}

impl DividendWithoutPaidTax {
    pub fn upgrade(self, taxes: &mut HashMap<TaxId, Cash>) -> GenericResult<Dividend> {
        Ok(Dividend {
            date: self.date,
            issuer: self.issuer,
            amount: self.amount,
            paid_tax: self.tax_extractor.get_paid_tax(taxes)?,
        })
    }
}

pub trait DividendPaidTaxExtractor {
    fn get_paid_tax(&self, taxes: &mut HashMap<TaxId, Cash>) -> GenericResult<Cash>;
}