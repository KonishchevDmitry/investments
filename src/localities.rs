use std::collections::{BTreeMap, HashMap};
use std::ops::Bound;

use chrono::{Datelike, Duration};

use crate::currency::Cash;
use crate::exchanges::Exchange;
use crate::taxes::{IncomeType, TaxRate, FixedTaxRate, NonUniformTaxRate};
use crate::types::{Date, Decimal};

#[derive(Clone)]
pub struct Country {
    pub jurisdiction: Jurisdiction,
    pub currency: &'static str,
    tax_rates: BTreeMap<i32, Box<dyn TaxRate>>,
}

impl Country {
    // FIXME(konishchev): Rewrite
    fn new(
        jurisdiction: Jurisdiction, mut default_tax_rate: Decimal,
        mut tax_rates: HashMap<IncomeType, BTreeMap<i32, Decimal>>,
    ) -> Country {
        let traits = jurisdiction.traits();

        default_tax_rate /= dec!(100);

        for tax_rates in tax_rates.values_mut() {
            for tax_rate in tax_rates.values_mut() {
                *tax_rate /= dec!(100);
            }
        }

        let mut tax_rate_spec = BTreeMap::<i32, HashMap<IncomeType, Decimal>>::new();

        for (&income_type, years) in &tax_rates {
            for (&year, &rate) in years {
                tax_rate_spec.entry(year).or_default().insert(income_type, rate);
            }
        }

        let mut tax_rates_new: BTreeMap<i32, Box<dyn TaxRate>> = tax_rate_spec.into_iter().map(|(year, rates)| {
            (year, Box::new(NonUniformTaxRate::new(rates, traits.tax_precision)) as Box<dyn TaxRate>)
        }).collect();

        tax_rates_new.insert(i32::MIN, Box::new(FixedTaxRate::new(default_tax_rate, traits.tax_precision)));

        Country {currency: traits.currency, jurisdiction, tax_rates: tax_rates_new}
    }

    pub fn cash(&self, amount: Decimal) -> Cash {
        Cash::new(self.currency, amount)
    }

    pub fn tax_rate(&self, year: i32) -> Box<dyn TaxRate> {
        self.tax_rates.range((Bound::Unbounded, Bound::Included(year))).last().unwrap().1.clone()
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

pub fn russia(
    trading_tax_rates: &BTreeMap<i32, Decimal>, dividends_tax_rates: &BTreeMap<i32, Decimal>,
    interest_tax_rates: &BTreeMap<i32, Decimal>,
) -> Country {
    Country::new(Jurisdiction::Russia, dec!(13), hashmap!{
        IncomeType::Trading => trading_tax_rates.clone(),
        IncomeType::Dividends => dividends_tax_rates.clone(),
        IncomeType::Interest => interest_tax_rates.clone(),
    })
}

pub fn get_russian_central_bank_min_last_working_day(today: Date) -> Date {
    // New Year holidays
    if today.month() == 1 && today.day() < 12 {
        std::cmp::max(
            today - Duration::days(10),
            date!(today.year() - 1, 12, 30),
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
    if date >= date!(2024, 08, 16) && false { // FIXME(konishchev): Enable it
        dec!(0.3)
    } else {
        dec!(0.1)
    }
}

pub fn deduce_us_dividend_amount(date: Date, result_income: Cash) -> Cash {
    let tax_rate = us_dividend_tax_rate(date);
    (result_income / (dec!(1) - tax_rate)).round()
}