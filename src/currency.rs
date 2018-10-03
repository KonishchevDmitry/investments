use std::collections::HashSet;
use std::ops::Deref;
use std::ptr;
use std::str::FromStr;
use std::sync::Mutex;

use core::GenericResult;
use types::Decimal;

lazy_static! {
    static ref CURRENCIES: Mutex<HashSet<&'static str>> = Mutex::new(HashSet::new());
}

#[derive(Debug)]
pub struct Cash {
    currency: &'static str,
    amount: Decimal,
}

impl Cash {
    pub fn new(currency: &str, amount: Decimal) -> Cash {
        Cash {
            currency: get_currency(currency),
            amount: amount,
        }
    }

    pub fn new_from_string(currency: &str, amount: &str) -> GenericResult<Cash> {
        Ok(Cash::new(currency, Decimal::from_str(amount).map_err(|_| format!(
            "Invalid cash amount: {:?}", amount))?))
    }
}

fn get_currency(currency: &str) -> &'static str {
    let mut currencies = CURRENCIES.lock().unwrap();

    match currencies.get(currency).map(|currency: &&str| *currency) {
        Some(static_currency) => static_currency,
        None => {
            let static_currency = Box::leak(currency.to_owned().into_boxed_str());
            currencies.insert(static_currency);
            static_currency
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn currency_cache() {
        let currencies = ["mock-1", "mock-2"];
        let mut cached_currencies = Vec::<&'static str>::new();

        for currency in currencies.iter().map(Deref::deref) {
            let cached_currency = get_currency(currency);
            cached_currencies.push(cached_currency);

            assert_eq!(cached_currency, currency);
            assert!(!ptr::eq(currency, cached_currency));
        }

        for (id, currency) in currencies.iter().enumerate() {
            assert!(ptr::eq(get_currency(currency), cached_currencies[id]));
        }
    }
}