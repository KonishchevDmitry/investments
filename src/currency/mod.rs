use std::fmt;
use std::str::FromStr;
use std::ops::{Mul, Neg};

use num_traits::identities::Zero;
use rust_decimal::RoundingStrategy;

use core::GenericResult;
use types::{Date, Decimal};

mod cbr;
mod name_cache;
mod rate_cache;

pub mod converter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cash {
    pub currency: &'static str,
    pub amount: Decimal,
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

    pub fn new_from_string_positive(currency: &str, amount: &str) -> GenericResult<Cash> {
        let cash = Cash::new_from_string(currency, amount)?;

        if !cash.is_positive() {
            return Err!("Invalid cash amount: {:?}", amount);
        }

        Ok(cash)
    }

    pub fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    pub fn is_positive(&self) -> bool {
        !self.amount.is_zero() && self.amount.is_sign_positive()
    }

    pub fn round(mut self) -> Cash {
        self.amount = round(self.amount);
        self
    }
}

impl Neg for Cash {
    type Output = Cash;

    fn neg(mut self) -> Cash {
        self.amount = -self.amount;
        self
    }
}

impl Mul<Decimal> for Cash {
    type Output = Cash;

    fn mul(mut self, rhs: Decimal) -> Cash {
        self.amount *= rhs;
        self
    }
}

impl fmt::Display for Cash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CashAssets {
    pub date: Date,
    pub cash: Cash,
}

impl CashAssets {
    #[cfg(test)]
    pub fn new(date: Date, currency: &str, amount: Decimal) -> CashAssets {
        CashAssets {date, cash: Cash::new(currency, amount)}
    }

    pub fn new_from_cash(date: Date, cash: Cash) -> CashAssets {
        CashAssets {date, cash}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrencyRate {
    date: Date,
    price: Decimal,
}

pub fn round(amount: Decimal) -> Decimal {
    amount.round_dp_with_strategy(2, RoundingStrategy::RoundHalfUp).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding() {
        for (from_string, to_string) in [
            ("1",     "1"),
            ("1.0",   "1"),
            ("1.1",   "1.1"),
            ("1.00",  "1"),
            ("1.01",  "1.01"),
            ("1.11",  "1.11"),
            ("1.004", "1"),
            ("1.005", "1.01"),
            ("1.111", "1.11"),
            ("1.114", "1.11"),
            ("1.124", "1.12"),
            ("1.115", "1.12"),
            ("1.125", "1.13"),
        ].iter() {
            let from = Decimal::from_str(from_string).unwrap();
            let to = Decimal::from_str(to_string).unwrap();

            let rounded = round(from);
            assert_eq!(rounded, to);

            assert_eq!(&from.to_string(), from_string);
            assert_eq!(&rounded.to_string(), to_string);
        }
    }
}