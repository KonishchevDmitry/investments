use std::collections::HashMap;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;
use crate::types::{Date, Decimal};

use super::payments::Payments;
use super::taxes::{TaxId, TaxAccruals};

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

#[derive(PartialEq, Eq, Hash)]
pub struct DividendId {
    pub date: Date,
    pub issuer: String,
    pub description: String,
    pub tax_description: Option<String>,
}

pub type DividendAccruals = Payments;

pub fn process_dividends(
    dividend: DividendId, accruals: DividendAccruals, taxes: &mut HashMap<TaxId, TaxAccruals>
) -> GenericResult<Option<Dividend>> {
    let paid_tax = dividend.tax_description.as_ref().map_or(Ok(None), |tax_description| {
        let tax_id = TaxId::new(dividend.date, &tax_description);
        let tax_accruals = taxes.remove(&tax_id).ok_or_else(|| format!(
            "There is no tax with {:?} expected description", tax_description
        ))?;

        tax_accruals.get_result().map_err(|e| format!(
            "Failed to process {} / {:?} tax: {}",
            formatting::format_date(dividend.date), tax_description, e))
    }).map_err(|e| format!(
        "Unable to match {} dividend from {} to paid taxes: {}",
        dividend.issuer, formatting::format_date(dividend.date), e)
    )?;

    let amount = accruals.get_result().map_err(|e| format!(
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