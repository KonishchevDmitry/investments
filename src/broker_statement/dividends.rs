use std::collections::HashMap;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;
use crate::taxes::IncomeType;
use crate::types::{Date, Decimal};

use super::payments::Payments;
use super::taxes::{TaxId, TaxAccruals};
use chrono::Datelike;

#[derive(Debug)]
pub struct Dividend {
    pub date: Date,
    pub issuer: String,
    pub amount: Cash,
    pub paid_tax: Cash,
}

impl Dividend {
    pub fn tax(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Decimal> {
        let amount = converter.convert_to_rounding(self.date, self.amount, country.currency)?;
        Ok(country.tax_to_pay(IncomeType::Dividends, self.date.year(), amount, None))
    }

    pub fn tax_to_pay(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Decimal> {
        let amount = converter.convert_to_rounding(self.date, self.amount, country.currency)?;
        let paid_tax = converter.convert_to_rounding(self.date, self.paid_tax, country.currency)?;
        Ok(country.tax_to_pay(IncomeType::Dividends, self.date.year(), amount, Some(paid_tax)))
    }

    pub fn description(&self) -> String {
        format!("{} dividend from {}", self.issuer, formatting::format_date(self.date))
    }
}

#[derive(PartialEq, Eq, Hash)]
pub struct DividendId {
    pub date: Date,
    pub issuer: String,
}

pub type DividendAccruals = Payments;

pub fn process_dividend_accruals(
    dividend: DividendId, accruals: DividendAccruals, taxes: &mut HashMap<TaxId, TaxAccruals>
) -> GenericResult<Option<Dividend>> {
    let tax_id = TaxId::new(dividend.date, &dividend.issuer);
    let paid_tax = taxes.remove(&tax_id).map_or(Ok(None), |tax_accruals| {
        tax_accruals.get_result().map_err(|e| format!(
            "Failed to process {} tax from {}: {}",
            tax_id.issuer, formatting::format_date(tax_id.date), e))
    })?;

    let amount = match accruals.get_result().map_err(|e| format!(
        "Failed to process {} dividend from {}: {}",
        dividend.issuer, formatting::format_date(dividend.date), e
    ))? {
        Some(amount) => amount,
        None => {
            if paid_tax.is_some() {
                return Err!("Got paid tax for reversed {} dividend from {}",
                            dividend.issuer, formatting::format_date(dividend.date));
            }

            return Ok(None);
        }
    };

    Ok(Some(Dividend {
        date: dividend.date,
        issuer: dividend.issuer,
        amount: amount,
        paid_tax: paid_tax.unwrap_or_else(|| Cash::new(amount.currency, dec!(0))),
    }))
}