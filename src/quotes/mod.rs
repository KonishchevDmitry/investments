use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

#[cfg(not(test))] use chrono::{DateTime, TimeZone};
use lazy_static::lazy_static;
use log::debug;
use regex::Regex;

use crate::config::Config;
use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::db;
#[cfg(not(test))] use crate::util;

use self::cache::Cache;
use self::finnhub::Finnhub;
use self::moex::Moex;
use self::twelvedata::TwelveData;

mod alphavantage;
mod cache;
mod finnhub;
mod moex;
mod twelvedata;

pub struct Quotes {
    cache: Cache,
    providers: Vec<Box<dyn QuotesProvider>>,
    batched_symbols: RefCell<HashSet<String>>,
}

impl Quotes {
    pub fn new(config: &Config, database: db::Connection) -> GenericResult<Quotes> {
        let finnhub = config.finnhub.as_ref().ok_or(
            "Finnhub configuration is not set in the configuration file")?;

        let twelvedata = config.twelvedata.as_ref().ok_or(
            "Twelve Data configuration is not set in the configuration file")?;

        Ok(Quotes::new_with(Cache::new(database, config.cache_expire_time, true), vec![
            Box::new(Finnhub::new(&finnhub.token)),
            Box::new(TwelveData::new(&twelvedata.token)),
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

    pub fn batch(&self, symbol: &str) -> EmptyResult {
        let mut symbol = symbol.to_owned();

        // Reverse pair quote sometimes slightly differs from `1 / pair`, but in some places we use
        // redundant currency conversions back and forth assuming that eventual result won't differ
        // more than rounding precision (for example in stock selling simulation when user specifies
        // base currency to calculate the performance in).
        //
        // To workaround the issue we make quotes consistent here.
        if let Ok((base, quote)) = parse_currency_pair(&symbol) {
            if base < quote {
                symbol = get_currency_pair(quote, base)
            }
        }

        if self.cache.get(&symbol)?.is_none() {
            self.batched_symbols.borrow_mut().insert(symbol);
        }

        Ok(())
    }

    pub fn get(&self, symbol: &str) -> GenericResult<Cash> {
        if let Some(price) = self.cache.get(symbol)? {
            return Ok(price);
        }

        self.batch(symbol)?;
        let mut batched_symbols = self.batched_symbols.borrow_mut();

        for provider in &self.providers {
            let quotes = {
                let symbols: Vec<&str> = batched_symbols.iter().filter_map(|symbol| {
                    let is_currency_pair = is_currency_pair(&symbol);

                    if
                        provider.supports_stocks() && !is_currency_pair ||
                        provider.supports_forex() && is_currency_pair
                    {
                        Some(symbol.as_str())
                    } else {
                        None
                    }
                }).collect();

                if symbols.is_empty() {
                    continue;
                }

                debug!("Getting quotes from {} for the following symbols: {}...",
                       provider.name(), symbols.join(", "));

                provider.get_quotes(&symbols).map_err(|e| format!(
                    "Failed to get quotes from {}: {}", provider.name(), e))?
            };

            for (symbol, price) in quotes.iter() {
                let mut price = *price;

                // Some providers return stock quotes with unnecessary very high precision, so add
                // rounding here. But don't round Forex pairs since we always round conversion
                // result + reverse pairs always need high precision.
                if provider.high_precision() && !is_currency_pair(symbol) {
                    let rounded_price = price.round();
                    let round_precision = (price.amount - rounded_price.amount).abs() / price.amount;

                    if round_precision < dec!(0.0001) {
                        price = rounded_price;
                    }
                };

                if let Ok((base, quote)) = parse_currency_pair(&symbol) {
                    let reverse_pair = get_currency_pair(quote, base);
                    let reverse_price = Cash::new(base, dec!(1) / price.amount);
                    self.cache.save(&reverse_pair, reverse_price)?;
                }

                self.cache.save(&symbol, price)?;
                batched_symbols.remove(symbol);
            }

            if batched_symbols.is_empty() {
                break;
            }
        }

        if !batched_symbols.is_empty() {
            let symbols = batched_symbols.iter().cloned().collect::<Vec<String>>();
            return Err!("Unable to find quotes for following symbols: {}", symbols.join(", "));
        }

        Ok(self.cache.get(symbol)?.unwrap())
    }
}

type QuotesMap = HashMap<String, Cash>;

trait QuotesProvider {
    fn name(&self) -> &'static str;
    fn supports_stocks(&self) -> bool {true}
    fn supports_forex(&self) -> bool {true}
    fn high_precision(&self) -> bool {false}
    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap>;
}

pub fn get_currency_pair(base: &str, quote: &str) -> String {
    format!("{}/{}", base, quote)
}

fn is_currency_pair(symbol: &str) -> bool {
    parse_currency_pair(symbol).is_ok()
}

fn parse_currency_pair(pair: &str) -> GenericResult<(&str, &str)> {
    lazy_static! {
        static ref REGEX: Regex = Regex::new(r"^(?P<base>[A-Z]{3})/(?P<quote>[A-Z]{3})$").unwrap();
    }

    let captures = REGEX.captures(pair).ok_or_else(|| format!(
        "Invalid currency pair: {:?}", pair))?;

    Ok((
        captures.name("base").unwrap().as_str(),
        captures.name("quote").unwrap().as_str(),
    ))
}

#[cfg(not(test))]
fn is_outdated_quote<T: TimeZone>(date_time: DateTime<T>) -> bool {
    (util::utc_now() - date_time.naive_utc()).num_days() >= 5
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

            fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
                let mut symbols = symbols.to_vec();
                symbols.sort_unstable();

                assert_eq!(*self.request_id.borrow(), 0);
                assert_eq!(&symbols, &["BND", "BNDX", "VTI"]);
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

            fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
                assert_eq!(*self.request_id.borrow(), 0);
                assert_eq!(symbols, ["BNDX"]);
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

        quotes.batch("VTI").unwrap();
        quotes.batch("BNDX").unwrap();
        assert_eq!(quotes.get("BND").unwrap(), Cash::new("USD", dec!(12.34)));

        quotes.batch("VXUS").unwrap();
        assert_eq!(quotes.get("BND").unwrap(), Cash::new("USD", dec!(12.34)));
        assert_eq!(quotes.get("VTI").unwrap(), Cash::new("USD", dec!(56.78)));
        assert_eq!(quotes.get("BNDX").unwrap(), Cash::new("USD", dec!(90.12)));
    }
}