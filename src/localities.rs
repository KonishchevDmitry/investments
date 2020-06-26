use chrono::{Datelike, Duration};

use num_traits::Zero;

use crate::currency;
use crate::types::{Date, Decimal};

#[derive(Clone, Copy)]
pub struct Country {
    pub currency: &'static str,
    tax_rate: Decimal,
    tax_precision: u32,
}

impl Country {
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

    pub fn round_tax(&self, tax: Decimal) -> Decimal {
        currency::round_to(currency::round(tax), self.tax_precision)
    }

    pub fn tax_to_pay(&self, income: Decimal, paid_tax: Option<Decimal>) -> Decimal {
        let income = currency::round(income);

        if income.is_sign_negative() || income.is_zero() {
            return dec!(0);
        }

        let tax_to_pay = self.round_tax(income * self.tax_rate);

        if let Some(paid_tax) = paid_tax {
            assert!(!paid_tax.is_sign_negative());
            let tax_deduction = self.round_tax(paid_tax);

            if tax_deduction < tax_to_pay {
                tax_to_pay - tax_deduction
            } else {
                dec!(0)
            }
        } else {
            tax_to_pay
        }
    }
}

pub fn russia() -> Country {
    Country {
        currency: "RUB",
        tax_rate: Decimal::new(13, 2),
        tax_precision: 0,
    }
}

pub fn is_valid_execution_date(conclusion: Date, execution: Date) -> bool {
    let expected_execution = conclusion + Duration::days(2);
    conclusion <= execution && get_russian_stock_exchange_min_last_working_day(execution) <= expected_execution
}

pub fn get_russian_stock_exchange_min_last_working_day(today: Date) -> Date {
    // New Year holidays
    if today.month() == 1 && today.day() < 10 {
        Date::from_ymd(today.year() - 1, 12, 30)
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