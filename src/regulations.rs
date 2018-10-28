use chrono::Datelike;

use types::{Date, Decimal};

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