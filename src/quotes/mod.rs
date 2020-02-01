use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use log::debug;

use crate::config::Config;
use crate::core::GenericResult;
use crate::currency::Cash;
use crate::db;

use self::cache::Cache;
use self::finnhub::Finnhub;
use self::moex::Moex;

mod alphavantage;
mod cache;
mod finnhub;
mod moex;

pub type QuotesRc = Rc<Quotes>;

pub struct Quotes {
    cache: Cache,
    providers: Vec<Box<dyn QuotesProvider>>,
    batched_symbols: RefCell<HashSet<String>>,
}

impl Quotes {
    pub fn new(config: &Config, database: db::Connection) -> GenericResult<Quotes> {
        let finnhub = config.finnhub.as_ref().ok_or(
            "Finnhub configuration is not set in the configuration file")?;

        Ok(Quotes::new_with(Cache::new(database, config.cache_expire_time), vec![
            Box::new(Finnhub::new(&finnhub.token)),
            Box::new(Moex::new()),
        ]))
    }

    fn new_with(cache: Cache, providers: Vec<Box<dyn QuotesProvider>>) -> Quotes {
        Quotes {
            cache: cache,
            providers: providers,
            batched_symbols: RefCell::new(HashSet::new()),
        }
    }

    pub fn batch(&self, symbol: &str) {
        self.batched_symbols.borrow_mut().insert(symbol.to_owned());
    }

    pub fn get(&self, symbol: &str) -> GenericResult<Cash> {
        if let Some(price) = self.cache.get(symbol)? {
            return Ok(price);
        }

        self.batch(symbol);
        let mut batched_symbols = self.batched_symbols.borrow_mut();

        let mut price = None;

        for provider in &self.providers {
            let symbols: Vec<String> = batched_symbols.iter().cloned().collect();

            debug!("Getting quotes from {} for the following symbols: {}...",
                   provider.name(), symbols.join(", "));
            let quotes = provider.get_quotes(&symbols)?;

            for (other_symbol, other_price) in quotes.iter() {
                if *other_symbol == symbol {
                    price.replace(*other_price);
                }

                self.cache.save(&other_symbol, *other_price)?;
                batched_symbols.remove(other_symbol);
            }

            if batched_symbols.is_empty() {
                break;
            }
        }

        if !batched_symbols.is_empty() {
            let symbols = batched_symbols.iter().cloned().collect::<Vec<String>>();
            return Err!("Unable to find quotes for following symbols: {}", symbols.join(", "));
        }

        Ok(price.unwrap())
    }
}

type QuotesMap = HashMap<String, Cash>;

trait QuotesProvider {
    fn name(&self) -> &'static str;
    fn get_quotes(&self, symbols: &[String]) -> GenericResult<QuotesMap>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache() {
        struct FirstProvider {
            request_id: RefCell<usize>,
        }

        impl QuotesProvider for FirstProvider {
            fn name(&self) -> &'static str {
                "first-provider"
            }

            fn get_quotes(&self, symbols: &[String]) -> GenericResult<QuotesMap> {
                let mut symbols = symbols.to_vec();
                symbols.sort();

                assert_eq!(*self.request_id.borrow(), 0);
                assert_eq!(symbols, vec![s!("BND"), s!("BNDX"), s!("VTI")]);
                *self.request_id.borrow_mut() += 1;

                let mut quotes = HashMap::new();
                quotes.insert(s!("BND"), Cash::new("USD", dec!(12.34)));
                quotes.insert(s!("VTI"), Cash::new("USD", dec!(56.78)));
                Ok(quotes)
            }
        }

        struct SecondProvider {
            request_id: RefCell<usize>,
        }

        impl QuotesProvider for SecondProvider {
            fn name(&self) -> &'static str {
                "second-provider"
            }

            fn get_quotes(&self, symbols: &[String]) -> GenericResult<QuotesMap> {
                assert_eq!(*self.request_id.borrow(), 0);
                assert_eq!(symbols, [s!("BNDX")]);
                *self.request_id.borrow_mut() += 1;

                let mut quotes = HashMap::new();
                quotes.insert(s!("BNDX"), Cash::new("USD", dec!(90.12)));
                Ok(quotes)
            }
        }

        let (_database, cache) = Cache::new_temporary();
        let quotes = Quotes::new_with(cache, vec![
            Box::new(FirstProvider {request_id: RefCell::new(0)}),
            Box::new(SecondProvider {request_id: RefCell::new(0)}),
        ]);

        quotes.batch("VTI");
        quotes.batch("BNDX");
        assert_eq!(quotes.get("BND").unwrap(), Cash::new("USD", dec!(12.34)));

        quotes.batch("VXUS");
        assert_eq!(quotes.get("BND").unwrap(), Cash::new("USD", dec!(12.34)));
        assert_eq!(quotes.get("VTI").unwrap(), Cash::new("USD", dec!(56.78)));
        assert_eq!(quotes.get("BNDX").unwrap(), Cash::new("USD", dec!(90.12)));
    }
}