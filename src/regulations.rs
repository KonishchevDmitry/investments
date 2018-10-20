use types::Decimal;

pub struct Country {
    pub currency: &'static str,
    pub tax_rate: Decimal,
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