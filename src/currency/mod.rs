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

    pub fn is_negative(&self) -> bool {
        !self.amount.is_zero() && self.amount.is_sign_negative()
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

    pub fn add_convert(self, date: Date, amount: Cash, converter: &CurrencyConverter) -> GenericResult<Cash> {
        self.add(converter.convert_to_cash(date, amount, self.currency)?)
    }

    pub fn add_convert_assign(&mut self, date: Date, amount: Cash, converter: &CurrencyConverter) -> EmptyResult {
        self.add_assign(converter.convert_to_cash(date, amount, self.currency)?)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, amount: Cash) -> GenericResult<Cash> {
        self.add(-amount)
    }

    pub fn sub_convert(self, date: Date, amount: Cash, converter: &CurrencyConverter) -> GenericResult<Cash> {
        self.add_convert(date, -amount, converter)
    }

    pub fn sub_convert_assign(&mut self, date: Date, amount: Cash, converter: &CurrencyConverter) -> EmptyResult {
        self.add_convert_assign(date, -amount, converter)
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
        let mut amount = self.amount.normalize();

        if amount.scale() == 1 {
            amount.set_scale(0).unwrap();
            amount = Decimal::new(amount.to_i64().unwrap() * 10, 2)
        }

        write!(f, "{}", format_currency(self.currency, &separated_float!(amount.to_string())))
    }
}

#[derive(Debug, Clone, Copy)]
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

    pub fn iter(&self) -> impl Iterator<Item=Cash> + '_ {
        self.assets.iter().map(|(&currency, &amount)| {
            Cash::new(currency, amount)
        })
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
    util::round(amount, 2)
}

pub fn round_to(amount: Decimal, points: u32) -> Decimal {
    util::round(amount, points)
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

    #[test]
    fn currency_formatting() {
        assert_eq!(Cash::new("USD", dec!(12.345)).to_string(), "$12.345");
        assert_eq!(Cash::new("USD", dec!(-12.345)).to_string(), "-$12.345");

        assert_eq!(Cash::new("RUB", dec!(12.345)).to_string(), "12.345₽");
        assert_eq!(Cash::new("RUB", dec!(-12.345)).to_string(), "-12.345₽");

        assert_eq!(Cash::new("UNKNOWN", dec!(12.345)).to_string(), "12.345 UNKNOWN");
        assert_eq!(Cash::new("UNKNOWN", dec!(-12.345)).to_string(), "-12.345 UNKNOWN");
    }

    #[rstest(input, expected,
        case("12",     "12"),
        case("12.3",   "12.30"),
        case("12.30",  "12.30"),
        case("12.34",  "12.34"),
        case("12.345", "12.345"),
        case("12.001", "12.001"),
    )]
    fn cash_formatting(input: &str, expected: &str) {
        let currency = "CURRENCY";

        for sign in &["", "-"] {
            let input = Cash::new(currency, Decimal::from_str(&format!("{}{}", sign, input)).unwrap());
            let expected = format!("{}{} {}", sign, expected, currency);
            assert_eq!(input.to_string(), expected);
        }
    }
}