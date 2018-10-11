use std::str::FromStr;

use core::GenericResult;
use types::{Date, Decimal};

mod cbr;
mod name_cache;
mod rate_cache;

pub mod converter;

#[derive(Debug, Clone, Copy)]
pub struct Cash {
    currency: &'static str,
    amount: Decimal,
}

impl Cash {
    pub fn new(currency: &str, amount: Decimal) -> Cash {
        Cash {
            currency: name_cache::get(currency),
            amount: amount,
        }
    }

    pub fn new_from_string(currency: &str, amount: &str) -> GenericResult<Cash> {
        Ok(Cash::new(currency, Decimal::from_str(amount).map_err(|_| format!(
            "Invalid cash amount: {:?}", amount))?))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CacheAssets {
    pub date: Date,
    pub cash: Cash,
}

impl CacheAssets {
    pub fn new(date: Date, currency: &str, amount: Decimal) -> CacheAssets {
        CacheAssets {date, cash: Cash::new(currency, amount)}
    }

    pub fn new_from_cash(date: Date, cash: Cash) -> CacheAssets {
        CacheAssets {date, cash}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrencyRate {
    date: Date,
    price: Decimal,
}