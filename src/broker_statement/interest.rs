use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::localities::Country;
use crate::taxes::{IncomeType, TaxCalculator};
use crate::time::Date;
use chrono::Datelike;

pub struct IdleCashInterest {
    pub date: Date,
    pub amount: Cash, // May be negative
}

impl IdleCashInterest {
    pub fn new(date: Date, amount: Cash) -> IdleCashInterest {
        IdleCashInterest {
            date, amount
        }
    }

    pub fn tax(&self, country: &Country, converter: &CurrencyConverter, calculator: &mut TaxCalculator) -> GenericResult<Cash> {
        let amount = converter.convert_to_cash_rounding(self.date, self.amount, country.currency)?;
        Ok(calculator.add_income(IncomeType::Interest, self.date.year(), amount, None).expected)
    }
}