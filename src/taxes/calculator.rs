use crate::core::GenericResult;
use crate::currency::Cash;
use crate::localities::Country;
use crate::taxes::IncomeType;

pub struct Tax {
    pub expected: Cash,
    pub paid: Cash,
    pub deduction: Cash,
    pub to_pay: Cash,
}

// FIXME(konishchev): A prototype/stub of progressive tax calculator
#[derive(Clone)]
pub struct TaxCalculator {
    // FIXME(konishchev): Remove pub here
    pub country: Country,
}

impl TaxCalculator {
    pub fn new(country: Country) -> TaxCalculator {
        TaxCalculator {
            country,
        }
    }

    pub fn tax_income(&mut self, income_type: IncomeType, year: i32, income: Cash, paid_tax: Option<Cash>) -> Tax {
        self.tax_income_dry_run(income_type, year, income, paid_tax)
    }

    pub fn tax_deductible_income(&mut self, income_type: IncomeType, year: i32, income: Cash, taxable_income: Cash) -> Tax {
        assert!(taxable_income <= income || taxable_income.is_zero() && income.is_negative()); // FIXME(konishchev): Do we need it?

        let expected = self.tax_income_dry_run(income_type, year, income, None).expected;
        let to_pay = self.tax_income(income_type, year, taxable_income, None).expected;
        assert!(to_pay <= expected);

        Tax {
            expected,
            paid: Cash::zero(self.country.currency),
            deduction: expected - to_pay, // FIXME(konishchev): Different meaning with dividends
            to_pay: to_pay,
        }
    }

    pub fn tax_income_dry_run(&self, income_type: IncomeType, year: i32, income: Cash, paid_tax: Option<Cash>) -> Tax {
        let expected = self.country.tax_to_pay(income_type, year, income, None);
        let paid = paid_tax.unwrap_or_else(|| Cash::zero(self.country.currency));
        let to_pay = self.country.tax_to_pay(income_type, year, income, paid_tax);

        Tax {
            expected: expected,
            paid: paid,
            deduction: expected - to_pay,
            to_pay: to_pay,
        }
    }

    pub fn tax_agent_income(&mut self, _income_type: IncomeType, _year: i32, _income: Cash, paid_tax: Cash) -> GenericResult<Tax> {
        if paid_tax.currency != self.country.currency {
            return Err!("Got withheld tax in an unexpected currency: {}", paid_tax.currency)
        }

        let provided_paid_tax = paid_tax;
        let paid_tax = self.country.round_tax(paid_tax);

        if provided_paid_tax != paid_tax {
            return Err!("Got an unexpected withheld tax: {} vs {}", provided_paid_tax, paid_tax);
        }

        // FIXME(konishchev): Re-thing its logic
        Ok(Tax {
            expected: paid_tax,
            paid: paid_tax,
            deduction: paid_tax,
            to_pay: Cash::zero(self.country.currency),
        })
    }
}