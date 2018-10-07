use std::collections::HashSet;
use std::ops::Deref;
use std::ptr;
use std::sync::Mutex;

lazy_static! {
    static ref CURRENCIES: Mutex<HashSet<&'static str>> = Mutex::new(HashSet::new());
}

pub fn get(currency: &str) -> &'static str {
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
    fn name_cache() {
        let currencies = ["mock-1", "mock-2"];
        let mut cached_currencies = Vec::<&'static str>::new();

        for currency in currencies.iter().map(Deref::deref) {
            let cached_currency = get(currency);
            cached_currencies.push(cached_currency);

            assert_eq!(cached_currency, currency);
            assert!(!ptr::eq(currency, cached_currency));
        }

        for (id, currency) in currencies.iter().enumerate() {
            assert!(ptr::eq(get(currency), cached_currencies[id]));
        }
    }
}