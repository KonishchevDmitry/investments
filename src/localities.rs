use std::collections::{HashMap, BTreeMap};
use std::ops::Bound;

use chrono::{Datelike, Duration};

use crate::currency::Cash;
use crate::exchanges::Exchange;
use crate::taxes::IncomeType;
use crate::types::{Date, Decimal};

#[derive(Clone)]
pub struct Country {
    pub currency: &'static str,
    default_tax_rate: Decimal,
    tax_rates: HashMap<IncomeType, BTreeMap<i32, Decimal>>,
    tax_precision: u32, // FIXME(konishchev): Deprecate it?
}

impl Country {
    fn new(
        currency: &'static str, mut default_tax_rate: Decimal,
        mut tax_rates: HashMap<IncomeType, BTreeMap<i32, Decimal>>, tax_precision: u32,
    ) -> Country {
        default_tax_rate /= dec!(100);

        for tax_rates in tax_rates.values_mut() {
            for tax_rate in tax_rates.values_mut() {
                *tax_rate /= dec!(100);
            }
        }

        Country {currency, default_tax_rate, tax_rates, tax_precision}
    }

    pub fn cash(&self, amount: Decimal) -> Cash {
        Cash::new(self.currency, amount)
    }

    // FIXME(konishchev): Deprecated
    pub fn round_tax(&self, tax: Cash) -> Cash {
        assert_eq!(tax.currency, self.currency);
        tax.round().round_to(self.tax_precision)
    }

    // FIXME(konishchev): Deprecate it
    pub fn tax_to_pay(
        &self, income_type: IncomeType, year: i32, income: Cash, paid_tax: Option<Cash>,
    ) -> Cash {
        assert_eq!(income.currency, self.currency);

        let income = income.round();
        if income.is_negative() || income.is_zero() {
            return Cash::zero(self.currency);
        }

        let tax_to_pay = self.round_tax(income * self.tax_rate(income_type, year));

        if let Some(paid_tax) = paid_tax {
            assert!(!paid_tax.is_negative());
            assert_eq!(paid_tax.currency, tax_to_pay.currency);
            let tax_deduction = self.round_tax(paid_tax);

            if tax_deduction.amount < tax_to_pay.amount {
                tax_to_pay - tax_deduction
            } else {
                Cash::zero(self.currency)
            }
        } else {
            tax_to_pay
        }
    }

    pub fn deduce_income(&self, income_type: IncomeType, year: i32, result_income: Cash) -> Cash {
        assert_eq!(result_income.currency, self.currency);
        (result_income / (dec!(1) - self.tax_rate(income_type, year))).round()
    }

    fn tax_rate(&self, income_type: IncomeType, year: i32) -> Decimal {
        self.tax_rates.get(&income_type).and_then(|tax_rates| {
            tax_rates
                .range((Bound::Unbounded, Bound::Included(year)))
                .map(|entry| *entry.1)
                .last()
        }).unwrap_or(self.default_tax_rate)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Jurisdiction {
    Russia,
    Usa,
}

impl Jurisdiction {
    pub fn name(self) -> &'static str {
        match self {
            Jurisdiction::Russia => "Russia",
            Jurisdiction::Usa    => "USA",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Jurisdiction::Russia => "RU",
            Jurisdiction::Usa    => "US",
        }
    }

    pub fn tax_precision(self) -> u32 {
        match self {
            Jurisdiction::Russia => 0,
            Jurisdiction::Usa    => 2,
        }
    }
}

pub fn russia(
    trading_tax_rates: &BTreeMap<i32, Decimal>, dividends_tax_rates: &BTreeMap<i32, Decimal>,
    interest_tax_rates: &BTreeMap<i32, Decimal>,
) -> Country {
    Country::new("RUB", dec!(13), hashmap!{
        IncomeType::Trading => trading_tax_rates.clone(),
        IncomeType::Dividends => dividends_tax_rates.clone(),
        IncomeType::Interest => interest_tax_rates.clone(),
    }, Jurisdiction::Russia.tax_precision())
}

pub fn us() -> Country {
    Country::new("USD", dec!(0), hashmap!{
        IncomeType::Dividends => btreemap!{0 => dec!(10)},
    }, Jurisdiction::Usa.tax_precision())
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