use std::collections::{HashMap, BTreeMap};
use std::ops::Bound;

use chrono::{Datelike, Duration};

use crate::currency::Cash;
use crate::taxes::IncomeType;
use crate::time;
use crate::types::{Date, Decimal};

#[derive(Clone)]
pub struct Country {
    pub currency: &'static str,
    default_tax_rate: Decimal,
    tax_rates: HashMap<IncomeType, BTreeMap<i32, Decimal>>,
    tax_precision: u32,
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

    pub fn round_tax(&self, tax: Cash) -> Cash {
        assert_eq!(tax.currency, self.currency);
        tax.round().round_to(self.tax_precision)
    }

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
            Jurisdiction::Usa => "USA",
        }
    }
}

// When we work with taxes in Russia, the following rounding rules are applied:
// 1. Result of all calculations must be with kopecks precision
// 2. If we have income in foreign currency then:
//    2.1. Round it to cents
//    2.2. Convert to rubles using precise currency rate (65.4244 for example)
//    2.3. Round to kopecks
// 3. Taxes are calculated with rouble precision. But we should use double rounding here:
//    calculate them with kopecks precision first and then round to roubles.
//
// Декларация program allows to enter income only with kopecks precision - not bigger.
// It calculates tax for $10.64 income with 65.4244 currency rate as following:
// 1. income = round(10.64 * 65.4244, 2) = 696.12 (696.115616 without rounding)
// 2. tax = round(round(696.12 * 0.13, 2), 0) = 91 (90.4956 without rounding)
pub fn russia(
    trading_tax_rates: &BTreeMap<i32, Decimal>, dividends_tax_rates: &BTreeMap<i32, Decimal>,
    interest_tax_rates: &BTreeMap<i32, Decimal>,
) -> Country {
    Country::new("RUB", dec!(13), hashmap!{
        IncomeType::Trading => trading_tax_rates.clone(),
        IncomeType::Dividends => dividends_tax_rates.clone(),
        IncomeType::Interest => interest_tax_rates.clone(),
    }, 0)
}

pub fn us() -> Country {
    Country::new("USD", dec!(0), hashmap!{
        IncomeType::Dividends => btreemap!{0 => dec!(10)},
    }, 2)
}

pub fn is_valid_execution_date(conclusion: Date, execution: Date) -> bool {
    let expected_execution = conclusion + Duration::days(2);
    conclusion <= execution && get_russian_stock_exchange_min_last_working_day(execution) <= expected_execution
}

pub fn get_russian_stock_exchange_min_last_working_day(today: Date) -> Date {
    // New Year holidays
    if today.month() == 1 && today.day() < 12 {
        std::cmp::max(
            today - Duration::days(10),
            Date::from_ymd(today.year() - 1, 12, 30),
        )
    // 8 march holidays
    } else if today.month() == 3 && today.day() == 12 {
        today - Duration::days(4)
    // May holidays
    } else if today.month() == 5 && today.day() >= 3 && today.day() <= 13 {
        today - Duration::days(5)
    // COVID-19 pandemic
    } else if today.year() == 2020 && today.month() == 4 && today.day() <= 6 {
        Date::from_ymd(2020, 3, 28)
    } else {
        today - Duration::days(3)
    }
}

pub fn nearest_possible_account_close_date() -> Date {
    let execution_date = time::today_trade_execution_date();

    let mut close_date = execution_date;
    while get_russian_stock_exchange_min_last_working_day(close_date) < execution_date {
        close_date += Duration::days(1);
    }

    close_date
}