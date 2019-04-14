use std;
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::str::FromStr;
use std::ops::{Mul, Div, Neg};

use num_traits::identities::Zero;
use num_traits::ToPrimitive;

use separator::Separatable;

use crate::core::{GenericResult, EmptyResult};
use crate::types::{Date, Decimal};
use crate::util;

use self::converter::CurrencyConverter;

mod cbr;
mod name_cache;
mod rate_cache;

pub mod converter;

#[derive(Debug, Clone, Copy, PartialEq)]
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

    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, amount: Cash) -> GenericResult<Cash> {
        self.add_assign(amount)?;
        Ok(self)
    }

    pub fn add_assign(&mut self, amount: Cash) -> EmptyResult {
        self.ensure_same_currency(amount)?;
        self.amount += amount.amount;
        Ok(())
    }

    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, amount: Cash) -> GenericResult<Cash> {
        self.add(-amount)
    }

    pub fn sub_convert(self, date: Date, amount: Cash, converter: &CurrencyConverter) -> GenericResult<Cash> {
        let amount = converter.convert_to_cash(date, amount, self.currency)?;
        self.sub(amount)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn div(self, amount: Cash) -> GenericResult<Decimal> {
        self.ensure_same_currency(amount)?;
        Ok(self.amount / amount.amount)
    }

    pub fn round(mut self) -> Cash {
        self.amount = round(self.amount);
        self
    }

    pub fn format(&self) -> String {
        format_currency(self.currency, &self.amount.to_string())
    }

    pub fn format_rounded(&self) -> String {
        let amount = round_to(self.amount, 0).to_i64().unwrap().separated_string();
        format_currency(self.currency, &amount)
    }

    fn ensure_same_currency(self, other: Cash) -> EmptyResult {
        if self.currency == other.currency {
            Ok(())
        } else {
            Err!("Currency mismatch: {} vs {}", self.currency, other.currency)
        }
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

    pub fn clear(&mut self) {
        self.assets.clear();
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

pub fn round_to(amount: Decimal, points: u32) -> Decimal {
    util::round_to(amount, points)
}

fn format_currency(currency: &str, mut amount: &str) -> String {
    let mut buffer = String::new();

    if currency == "USD" {
        if amount.starts_with('-') || amount.starts_with('+') {
            write!(&mut buffer, "{}", &amount[..1]).unwrap();
            amount = &amount[1..];
        }

        write!(&mut buffer, "$").unwrap();
    }

    write!(&mut buffer, "{}", amount).unwrap();

    match currency {
        "USD" => (),
        "RUB" => write!(&mut buffer, "₽").unwrap(),
        _ => write!(&mut buffer, " {}", currency).unwrap(),
    };

    buffer
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

    #[test]
    fn formatting() {
        assert_eq!(Cash::new("USD", dec!(12.345)).format(), "$12.345");
        assert_eq!(Cash::new("USD", dec!(-12.345)).format(), "-$12.345");

        assert_eq!(Cash::new("RUB", dec!(12.345)).format(), "12.345₽");
        assert_eq!(Cash::new("RUB", dec!(-12.345)).format(), "-12.345₽");

        assert_eq!(Cash::new("UNKNOWN", dec!(12.345)).format(), "12.345 UNKNOWN");
        assert_eq!(Cash::new("UNKNOWN", dec!(-12.345)).format(), "-12.345 UNKNOWN");
    }
}