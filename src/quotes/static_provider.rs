use std::collections::HashMap;
use std::sync::Mutex;

use log::warn;
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::GenericResult;
use crate::currency::{self, Cash};
use crate::util::{self, DecimalRestrictions};

use super::{SupportedExchange, QuotesMap, QuotesProvider};

pub struct StaticProviderConfig(QuotesMap);

impl<'de> Deserialize<'de> for StaticProviderConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let value: HashMap<String, String> = Deserialize::deserialize(deserializer)?;

        let mut quotes = QuotesMap::new();

        for (symbol, price) in value {
            let price = parse_price(&price).ok_or_else(|| D::Error::custom(format!(
                "Invalid price: {price:?}")))?;

            quotes.insert(symbol, price);
        }

        Ok(StaticProviderConfig(quotes))
    }
}

fn parse_price(value: &str) -> Option<Cash> {
    let value = util::fold_spaces(value);
    let mut tokens = value.split(' ');

    let price = tokens.next().and_then(|price| {
        util::parse_decimal(price, DecimalRestrictions::StrictlyPositive).ok()
    })?;

    let currency = tokens.next().and_then(|currency| {
        currency::validate_currency(currency).ok()?;
        Some(currency)
    })?;

    if tokens.next().is_some() {
        return None;
    }

    Some(Cash::new(currency, price))
}

pub struct StaticProvider {
    quotes: QuotesMap,
    warned: Mutex<bool>,
}

impl StaticProvider {
    pub fn new(config: &StaticProviderConfig) -> StaticProvider {
        StaticProvider {
            quotes: config.0.clone(),
            warned: Mutex::new(false),
        }
    }
}

impl QuotesProvider for StaticProvider {
    fn name(&self) -> &'static str {
        "static quotes provider"
    }

    fn supports_stocks(&self) -> SupportedExchange {
        SupportedExchange::Any
    }

    fn supports_forex(&self) -> bool {
        true
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        let mut quotes = QuotesMap::new();

        for &symbol in symbols {
            if let Some(&price) = self.quotes.get(symbol) {
                quotes.insert(symbol.to_owned(), price);
            }
        }

        if !quotes.is_empty() {
            let mut warned = self.warned.lock().unwrap();
            if !*warned {
                warn!("Static quotes provider is used. The quotes will be outdated.");
                *warned = true;
            }
        }

        Ok(quotes)
    }
}