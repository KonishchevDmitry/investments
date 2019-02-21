use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::localities::Country;
use crate::types::{Date, Decimal};

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