use chrono::{Datelike, Duration};

use crate::currency;
use crate::types::{Date, Decimal};

pub struct Country {
    pub currency: &'static str,
    tax_rate: Decimal,
    tax_precision: u32,
}

impl Country {
    /// Returns an approximate date when tax is going to be paid for the specified income
    pub fn get_tax_payment_date(&self, income_date: Date) -> Date {
        Date::from_ymd(income_date.year() + 1, 3, 15)
    }

    pub fn tax_to_pay(&self, income: Decimal, paid_tax: Option<Decimal>) -> Decimal {
        // TODO: It looks like Декларация program rounds tax amount to rubles as
        // round_to(round_to(value, 2), 0) because it rounds 10.64 * 65.4244 * 0.13
        // (which is 90.4956) to 91.

        assert!(!income.is_sign_negative());
        let tax_to_pay = currency::round_to(income * self.tax_rate, self.tax_precision);

        if let Some(paid_tax) = paid_tax {
            assert!(!paid_tax.is_sign_negative());
            let tax_deduction = currency::round_to(paid_tax, self.tax_precision);

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

pub fn us() -> Country {
    Country {
        currency: "USD",
        tax_rate: Decimal::new(10, 2),
        tax_precision: 2,
    }
}

pub fn get_russian_stock_exchange_min_last_working_day(today: Date) -> Date {
    if today.month() == 1 && today.day() < 10 {
        Date::from_ymd(today.year() - 1, 12, 30)
    } else if (today.month() == 3 || today.month() == 5) && today.day() < 13 {
        today - Duration::days(5)
    } else {
        today - Duration::days(3)
    }
}