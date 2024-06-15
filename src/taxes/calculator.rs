use chrono::Datelike;

use crate::currency::Cash;
use crate::localities::Country;
use crate::taxes::IncomeType;
use crate::time::Date;

pub struct Tax {
    pub expected: Cash,
    pub paid: Cash,
    pub to_pay: Cash,
}

// FIXME(konishchev): A prototype/stub of progressive tax calculator
pub struct TaxCalculator {
    country: Country,
}

impl TaxCalculator {
    pub fn new(country: Country) -> TaxCalculator {
        TaxCalculator {
            country,
        }
    }

    pub fn add_income(&mut self, income_type: IncomeType, date: Date, income: Cash, paid_tax: Option<Cash>) -> Tax {
        Tax {
            expected: self.country.tax_to_pay(income_type, date.year(), income, None),
            paid: paid_tax.unwrap_or_else(|| Cash::zero(&self.country.currency)),
            to_pay: self.country.tax_to_pay(income_type, date.year(), income, paid_tax),
        }
    }

    // FIXME(konishchev): Re-thing its logic
    pub fn add_tax_agent_income(&mut self, _income_type: IncomeType, _date: Date, _income: Cash, paid_tax: Cash) -> Tax {
        Tax {
            expected: paid_tax,
            paid: paid_tax,
            to_pay: Cash::zero(&self.country.currency),
        }
    }
}