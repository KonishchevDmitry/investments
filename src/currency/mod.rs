use std;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::ops::{Mul, Div, Neg};

use num_traits::identities::Zero;

use core::GenericResult;
use types::{Date, Decimal};
use util;

use self::converter::CurrencyConverter;

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

    pub fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    pub fn is_positive(&self) -> bool {
        !self.amount.is_zero() && self.amount.is_sign_positive()
    }

    pub fn sub(&mut self, amount: Cash) {
        assert_eq!(self.currency, amount.currency);
        self.amount -= amount.amount;
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

impl<T> Mul<T> for Cash where T: Into<Decimal> {
    type Output = Cash;

    fn mul(mut self, rhs: T) -> Cash {
        self.amount *= rhs.into();
        self
    }
}

impl<T> Div<T> for Cash where T: Into<Decimal> {
    type Output = Cash;

    fn div(mut self, rhs: T) -> Cash {
        self.amount /= rhs.into();
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
    pub fn new_from_cash(date: Date, cash: Cash) -> CashAssets {
        CashAssets {date, cash}
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct CurrencyRate {
    date: Date,
    price: Decimal,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct MultiCurrencyCashAccount {
    assets: HashMap<&'static str, Decimal>,
}

impl MultiCurrencyCashAccount {
    pub fn new() -> MultiCurrencyCashAccount {
        MultiCurrencyCashAccount {
            assets: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn has_assets(&self, currency: &str) -> bool {
        self.assets.get(currency).is_some()
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<&'static str, Decimal> {
        self.assets.iter()
    }

    pub fn deposit(&mut self, amount: Cash) {
        if let Some(assets) = self.assets.get_mut(amount.currency) {
            *assets += amount.amount;
            return;
        }

        self.assets.insert(amount.currency, amount.amount);
    }

    pub fn withdraw(&mut self, amount: Cash) {
        self.deposit(-amount)
    }

    pub fn total_assets(&self, currency: &str, converter: &CurrencyConverter) -> GenericResult<Decimal> {
        let mut total_assets = dec!(0);

        for (other_currency, amount) in &self.assets {
            let assets = Cash::new(other_currency, *amount);
            total_assets += converter.convert_to(util::today(), assets, currency)?;
        }

        Ok(total_assets)
    }
}

pub fn round(amount: Decimal) -> Decimal {
    util::round_to(amount, 2)
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