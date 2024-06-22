use std::collections::HashMap;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::localities::{Country, Jurisdiction};
use crate::taxes::{self, IncomeType, TaxRate};

pub struct Tax {
    pub expected: Cash,
    pub paid: Cash,
    pub deduction: Cash,
    pub to_pay: Cash,
}

#[derive(Clone)] // FIXME(konishchev): Forbid it
pub struct TaxCalculator {
    pub country: Country,
    years: HashMap<i32, Box<dyn TaxRate>>,
}

impl TaxCalculator {
    pub fn new(country: Country) -> TaxCalculator {
        TaxCalculator {
            country,
            years: HashMap::new(),
        }
    }

    // Attention: Modifies calculator state. Must be called only for income that won't be decreased later via deductions
    // or looses balancing.
    pub fn tax_income(&mut self, income_type: IncomeType, year: i32, income: Cash, paid_tax: Option<Cash>) -> Tax {
        calculate(self.country.jurisdiction, self.year(year), income_type, income, paid_tax)
    }

    // Intended for dividends, tax for which was withheld by tax agent.
    pub fn tax_agent_income(&mut self, income_type: IncomeType, year: i32, income: Cash, mut paid_tax: Cash) -> GenericResult<Tax> {
        if paid_tax.currency != self.country.currency {
            return Err!("Got withheld tax in an unexpected currency: {}", paid_tax.currency)
        }

        let orig_paid_tax = paid_tax;
        paid_tax.amount = taxes::round_tax(paid_tax.amount, self.country.jurisdiction.traits().tax_precision);

        if orig_paid_tax != paid_tax {
            return Err!("Got an unexpected withheld tax: {} vs {}", orig_paid_tax, paid_tax);
        }

        // Increase total tax base, but trust the provided withheld tax amount
        self.tax_income(income_type, year, income, None);

        // FIXME(konishchev): Try to differentiate these situations
        // Please note that "to pay" amount we'll return here will be less than actual tax will be paid in case of
        // progressive tax rates:
        //
        // Withheld tax may be less than 13%. It may be even zero if company distributes dividends from other companies
        // for which tax has been already withheld, so we should trust the provided amount. But, in case of progressive
        // tax rates the withheld tax will be taken using lower rate, as broken doesn't know actual client's income.
        //
        // See https://web.archive.org/web/20240622133328/https://smart-lab.ru/company/tinkoff_invest/blog/631922.php
        // for details.
        Ok(Tax {
            expected: paid_tax,
            paid: paid_tax,
            deduction: paid_tax,
            to_pay: Cash::zero(self.country.currency),
        })
    }

    // Attention: Modifies calculator state. Must be called only for income that won't be decreased later via deductions
    // or looses balancing.
    pub fn tax_deductible_income(&mut self, income_type: IncomeType, year: i32, income: Cash, taxable_income: Cash) -> Tax {
        let country = self.country.jurisdiction;

        let calc = self.year(year);
        let mut dry_run_calc = calc.clone();

        let full = calculate(country, &mut dry_run_calc, income_type, income, None);
        let real = calculate(country, calc, income_type, taxable_income, None);

        assert!(real.paid.is_zero());
        assert_eq!(real.to_pay, real.expected);
        assert!(real.expected <= full.expected);

        Tax {
            expected: full.expected,
            paid: real.paid,
            deduction: full.expected - real.to_pay, // FIXME(konishchev): Different meaning with dividends
            to_pay: real.to_pay,
        }
    }

    // Attention: Always operates on clean calculator state. Intended for intermediate calculations during stock selling
    // processing which are processed before any looses balancing.
    pub fn tax_deductible_income_dry_run(&self, income_type: IncomeType, year: i32, income: Cash, taxable_income: Cash) -> Tax {
        let country = self.country.jurisdiction;

        let mut full_calc = self.country.tax_rate(year);
        let mut real_calc = full_calc.clone();

        let full = calculate(country, &mut full_calc, income_type, income, None);
        let real = calculate(country, &mut real_calc, income_type, taxable_income, None);

        assert!(real.paid.is_zero());
        assert_eq!(real.to_pay, real.expected);
        assert!(real.expected <= full.expected);

        Tax {
            expected: full.expected,
            paid: real.paid,
            deduction: full.expected - real.to_pay, // FIXME(konishchev): Different meaning with dividends
            to_pay: real.to_pay,
        }
    }

    fn year(&mut self, year: i32) -> &mut Box<dyn TaxRate> {
        self.years.entry(year).or_insert_with(|| self.country.tax_rate(year))
    }
}

fn calculate(
    jurisdiction: Jurisdiction, calc: &mut Box<dyn TaxRate>,
    income_type: IncomeType, income: Cash, paid_tax: Option<Cash>,
) -> Tax {
    let country = jurisdiction.traits();

    assert_eq!(income.currency, country.currency);
    let expected = calc.tax(income_type, income.amount);

    let (paid, to_pay) = if let Some(paid_tax) = paid_tax {
        assert!(!paid_tax.is_negative());
        assert_eq!(paid_tax.currency, country.currency);

        (
            paid_tax.amount,
            std::cmp::max(dec!(0), expected - taxes::round_tax(paid_tax.amount, country.tax_precision))
        )
    } else {
        (dec!(0), expected)
    };

    Tax {
        expected: Cash::new(country.currency, expected),
        paid: Cash::new(country.currency, paid),
        deduction: Cash::new(country.currency, expected - to_pay),  // FIXME(konishchev): Different meaning with dividends
        to_pay: Cash::new(country.currency, to_pay),
    }
}