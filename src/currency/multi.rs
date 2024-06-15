use std::collections::HashMap;

use crate::core::GenericResult;
use crate::currency::{Cash, converter::CurrencyConverter};
use crate::time::Date;
use crate::types::Decimal;

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
        self.assets.contains_key(currency)
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
            converter.batch(date, assets.currency, currency)?;
        }

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