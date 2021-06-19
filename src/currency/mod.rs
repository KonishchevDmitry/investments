#[cfg(test)] use std::str::FromStr;

use crate::time::Date;
use crate::types::Decimal;
use crate::util;

mod cash;
mod cbr;
mod multi;
mod name_cache;
mod rate_cache;

pub mod converter;

pub use self::cash::Cash;
pub use self::multi::MultiCurrencyCashAccount;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct CurrencyRate {
    date: Date,
    price: Decimal,
}

#[derive(Clone, Copy)]
pub struct CashAssets {
    pub date: Date,
    pub cash: Cash,
}

impl CashAssets {
    pub fn new(date: Date, currency: &str, amount: Decimal) -> CashAssets {
        CashAssets::new_from_cash(date, Cash::new(currency, amount))
    }

    pub fn new_from_cash(date: Date, cash: Cash) -> CashAssets {
        CashAssets {date, cash}
    }
}

pub fn round(amount: Decimal) -> Decimal {
    util::round(amount, 2)
}

pub fn round_to(amount: Decimal, points: u32) -> Decimal {
    util::round(amount, points)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(input, expected,
        case("1",     "1"),
        case("1.0",   "1"),
        case("1.1",   "1.1"),
        case("1.00",  "1"),
        case("1.01",  "1.01"),
        case("1.11",  "1.11"),
        case("1.004", "1"),
        case("1.005", "1.01"),
        case("1.111", "1.11"),
        case("1.114", "1.11"),
        case("1.124", "1.12"),
        case("1.115", "1.12"),
        case("1.125", "1.13"),
    )]
    fn rounding(input: &str, expected: &str) {
        let from = Decimal::from_str(input).unwrap();
        let to = Decimal::from_str(expected).unwrap();

        let rounded = round(from);
        assert_eq!(rounded, to);

        assert_eq!(&from.to_string(), input);
        assert_eq!(&rounded.to_string(), expected);
    }
}