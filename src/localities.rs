use chrono::{Datelike, Duration};

use crate::types::{Date, Decimal};

pub struct Country {
    pub currency: &'static str,
    pub tax_rate: Decimal,
}

impl Country {
    /// Returns an approximate date when tax is going to be paid for the specified income
    pub fn get_tax_payment_date(&self, income_date: Date) -> Date {
        Date::from_ymd(income_date.year() + 1, 3, 15)
    }
}

pub fn russia() -> Country {
    Country {
        currency: "RUB",
        tax_rate: Decimal::new(13, 2),
    }
}

pub fn us() -> Country {
    Country {
        currency: "USD",
        tax_rate: Decimal::new(10, 2),
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