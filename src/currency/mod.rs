use std::collections::HashMap;
#[cfg(test)] use std::str::FromStr;

use crate::core::GenericResult;
use crate::time::Date;
use crate::types::Decimal;
use crate::util;

use self::converter::CurrencyConverter;

mod cash;
mod cbr;
mod rate_cache;

pub mod converter;
pub mod name_cache;

pub use cash::Cash;

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

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct CurrencyRate {
    date: Date,
    price: Decimal,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct MultiCurrencyCashAccount {
    assets: HashMap<&'static str, Decimal>,
}

impl MultiCurrencyCashAccount {
    pub fn new() -> MultiCurrencyCashAccount {
        Default::default()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn has_assets(&self, currency: &str) -> bool {
        self.assets.get(currency).is_some()
    }

    pub fn get(&self, currency: &str) -> Option<Cash> {
        self.assets.get(currency).map(|&amount| Cash::new(currency, amount))
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

    pub fn add(&mut self, other: &MultiCurrencyCashAccount) {
        for amount in other.iter() {
            self.deposit(amount);
        }
    }

    pub fn total_assets(
        &self, date: Date, currency: &str, converter: &CurrencyConverter
    ) -> GenericResult<Decimal> {
        let mut total_assets = dec!(0);

        for assets in self.iter() {
            total_assets += converter.convert_to(date, assets, currency)?;
        }

        Ok(total_assets)
    }

    pub fn total_cash_assets(
        &self, date: Date, currency: &str, converter: &CurrencyConverter
    ) -> GenericResult<Cash> {
        Ok(Cash::new(currency, self.total_assets(date, currency, converter)?))
    }

    pub fn total_assets_real_time(
        &self, currency: &str, converter: &CurrencyConverter
    ) -> GenericResult<Decimal> {
        self.total_assets(converter.real_time_date(), currency, converter)
    }
}

impl From<Cash> for MultiCurrencyCashAccount {
    fn from(amount: Cash) -> MultiCurrencyCashAccount {
        let mut assets = MultiCurrencyCashAccount::new();
        assets.deposit(amount);
        assets
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