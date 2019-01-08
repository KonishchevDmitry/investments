#[cfg(test)] use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use log::debug;

use crate::config::Config;
use crate::core::GenericResult;
use crate::currency::Cash;
use crate::db;

use self::alphavantage::AlphaVantage;
use self::cache::Cache;
use self::moex::Moex;

mod alphavantage;
mod cache;
mod moex;

pub struct Quotes {
    cache: Cache,
    providers: Vec<Box<QuotesProvider>>,
    batched_symbols: HashSet<String>,
}

impl Quotes {
    pub fn new(config: &Config, database: db::Connection) -> Quotes {
        Quotes::new_with(Cache::new(database, config.cache_expire_time), vec![
            Box::new(AlphaVantage::new(&config.alphavantage.api_key)),
            Box::new(Moex::new()),
        ])
    }

    fn new_with(cache: Cache, providers: Vec<Box<QuotesProvider>>) -> Quotes {
        Quotes {
            cache: cache,
            providers: providers,
            batched_symbols: HashSet::new(),
        }
    }

    pub fn batch(&mut self, symbol: &str) {
        self.batched_symbols.insert(symbol.to_owned());
    }

    pub fn get(&mut self, symbol: &str) -> GenericResult<Cash> {
        if let Some(price) = self.cache.get(symbol)? {
            return Ok(price);
        }

        self.batch(symbol);
        let mut price = None;

        for provider in &self.providers {
            let symbols: Vec<String> = self.batched_symbols.iter()
                .map(|symbol| symbol.to_owned()).collect();

            debug!("Getting quotes from {} for the following symbols: {}...",
                   provider.name(), symbols.join(", "));
            let quotes = provider.get_quotes(&symbols)?;

            for (other_symbol, other_price) in quotes.iter() {
                if *other_symbol == symbol {
                    price = Some(*other_price);
                }

                self.cache.save(&other_symbol, *other_price)?;
                self.batched_symbols.remove(other_symbol);
            }

            if self.batched_symbols.is_empty() {
                break;
            }
        }

        if !self.batched_symbols.is_empty() {
            let symbols = self.batched_symbols.iter()
                .map(|symbol| symbol.to_owned()).collect::<Vec<String>>();
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
                quotes.insert(s!("BND"), Cash::new("USD", decf!(12.34)));
                quotes.insert(s!("VTI"), Cash::new("USD", decf!(56.78)));
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
                quotes.insert(s!("BNDX"), Cash::new("USD", decf!(90.12)));
                Ok(quotes)
            }
        }

        let (_database, cache) = Cache::new_temporary();
        let mut quotes = Quotes::new_with(cache, vec![
            Box::new(FirstProvider {request_id: RefCell::new(0)}),
            Box::new(SecondProvider {request_id: RefCell::new(0)}),
        ]);

        quotes.batch("VTI");
        quotes.batch("BNDX");
        assert_eq!(quotes.get("BND").unwrap(), Cash::new("USD", decf!(12.34)));

        quotes.batch("VXUS");
        assert_eq!(quotes.get("BND").unwrap(), Cash::new("USD", decf!(12.34)));
        assert_eq!(quotes.get("VTI").unwrap(), Cash::new("USD", decf!(56.78)));
        assert_eq!(quotes.get("BNDX").unwrap(), Cash::new("USD", decf!(90.12)));
    }
}