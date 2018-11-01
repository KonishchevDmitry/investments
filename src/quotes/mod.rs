use std::collections::{HashMap, HashSet};

use config::Config;
use core::GenericResult;
use currency::Cash;

use self::alphavantage::AlphaVantage;

mod alphavantage;
mod cache;

pub struct Quotes {
    providers: Vec<Box<QuotesProvider>>,
    batch: HashSet<String>,
}

impl Quotes {
    pub fn new(config: &Config) -> Quotes {
        Quotes {
            providers: vec![
                Box::new(AlphaVantage::new(&config.alphavantage.api_key)),
            ],
            batch: HashSet::new(),
        }
    }

    pub fn batch(&mut self, symbol: &str) {
        self.batch.insert(symbol.to_owned());
    }

    pub fn get(&mut self, symbol: &str) -> GenericResult<Cash> {
        self.batch(symbol);
        unreachable!();
    }
}

type QuotesMap = HashMap<String, Cash>;

trait QuotesProvider {
    fn get_quotes(&self, symbols: &Vec<String>) -> GenericResult<QuotesMap>;
}