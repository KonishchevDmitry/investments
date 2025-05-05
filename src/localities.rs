use std::collections::BTreeMap;
use std::rc::Rc;

use chrono::{Datelike, Duration};

use crate::currency::Cash;
use crate::exchanges::Exchange;
use crate::taxes::{FixedTaxRate, ProgressiveTaxRate, TaxConfig, TaxRate};
use crate::types::{Date, Decimal};

#[derive(Clone)]
pub struct Country {
    pub jurisdiction: Jurisdiction,
    pub currency: &'static str,
    tax_rates: Rc<BTreeMap<i32, Box<dyn TaxRate>>>,
    tax_agent_rates: Rc<BTreeMap<i32, Box<dyn TaxRate>>>,
}

impl Country {
    fn new(jurisdiction: Jurisdiction, tax_rates: BTreeMap<i32, Box<dyn TaxRate>>, tax_agent_rates: BTreeMap<i32, Box<dyn TaxRate>>) -> Country {
        Country {
            jurisdiction,
            currency: jurisdiction.traits().currency,
            tax_rates: Rc::new(tax_rates),
            tax_agent_rates: Rc::new(tax_agent_rates),
        }
    }

    pub fn cash(&self, amount: Decimal) -> Cash {
        Cash::new(self.currency, amount)
    }

    pub fn tax_rate(&self, year: i32) -> Box<dyn TaxRate> {
        self.tax_rates.range(..=year).last().unwrap().1.clone()
    }

    pub fn tax_agent_rate(&self, year: i32) -> Box<dyn TaxRate> {
        self.tax_agent_rates.range(..=year).last().unwrap().1.clone()
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Jurisdiction {
    Russia,
    Usa,
}

pub struct JurisdictionTraits {
    pub name: &'static str,
    pub code: &'static str,
    pub currency: &'static str,
    pub tax_precision: u32,
}

impl Jurisdiction {
    pub fn traits(self) -> JurisdictionTraits {
        match self {
            Jurisdiction::Russia => JurisdictionTraits{
                name: "Russia",
                code: "RU",
                currency: "RUB",
                tax_precision: 0,
            },
            Jurisdiction::Usa => JurisdictionTraits{
                name: "USA",
                code: "US",
                currency: "USD",
                tax_precision: 2,
            },
        }
    }
}

pub fn russia(config: &TaxConfig) -> Country {
    let jurisdiction = Jurisdiction::Russia;
    let tax_precision = jurisdiction.traits().tax_precision;

    // Starting from 2021 we had progressive tax rate with single tax base
    let rates_2021 = Rc::new(btreemap!{
        dec!(0) => dec!(0.13),
        dec!(5_000_000) => dec!(0.15),
    });

    // Starting from 2025 we've got progressive tax rate with two tax bases:
    // 1. Income from employment
    // 2. Dividends, interest, trading, property
    let rates_2025 = Rc::new(btreemap!{
        dec!(0) => dec!(0.13),
        dec!(2_400_000) => dec!(0.15),
    });

    let tax_agent_calculators = btreemap! {
        i32::MIN => Box::new(FixedTaxRate::new(dec!(0.13), tax_precision)) as Box<dyn TaxRate>,
        2021 => Box::new(ProgressiveTaxRate::new(dec!(0), rates_2021.clone(), tax_precision)) as Box<dyn TaxRate>,
        2025 => Box::new(ProgressiveTaxRate::new(dec!(0), rates_2025.clone(), tax_precision)) as Box<dyn TaxRate>,
    };

    let mut tax_calculators = tax_agent_calculators.clone();

    for (&year, &income) in config.income.range(2021..2025) {
        let calc = Box::new(ProgressiveTaxRate::new(income, rates_2021.clone(), tax_precision));
        tax_calculators.insert(year, calc);
    }

    Country::new(Jurisdiction::Russia, tax_calculators, tax_agent_calculators)
}

pub fn get_russian_central_bank_min_last_working_day(today: Date) -> Date {
    // New Year holidays
    if today.month() == 1 && today.day() < 12 {
        std::cmp::max(
            today - Duration::days(11),
            date!(today.year() - 1, 12, 29),
        )
    // COVID-19 pandemic
    } else if today.year() == 2020 && today.month() == 4 && today.day() <= 6 {
        date!(2020, 3, 28)
    // Weekends, 8 March, May and occasional COVID-19 pandemic holidays
    } else {
        today - Duration::days(5)
    }
}

pub fn get_nearest_possible_russian_account_close_date() -> Date {
    [Exchange::Moex, Exchange::Spb].iter().map(|exchange| {
        let execution_date = exchange.trading_mode().execution_date(crate::exchanges::today_trade_conclusion_time());

        let mut close_date = execution_date;
        while exchange.min_last_working_day(close_date) < execution_date {
            close_date += Duration::days(1);
        }

        close_date
    }).max().unwrap()
}

pub fn us_dividend_tax_rate(date: Date) -> Decimal {
    if date >= date!(2024, 8, 16) {
        dec!(0.3)
    } else {
        dec!(0.1)
    }
}

pub fn deduce_us_dividend_amount(date: Date, result_income: Cash) -> Cash {
    let tax_rate = us_dividend_tax_rate(date);
    (result_income / (dec!(1) - tax_rate)).round()
}