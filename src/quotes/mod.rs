pub mod alphavantage;
mod cache;
mod common;
mod custom_provider;
pub mod fcsapi;
mod finex;
pub mod finnhub;
mod moex;
mod static_provider;
pub mod tinkoff;
pub mod twelvedata;

use std::cell::RefCell;
use std::collections::{hash_map::Entry, HashMap};
use std::rc::Rc;
use std::sync::Arc;
#[cfg(test)] use std::sync::Mutex;

use itertools::Itertools;
use log::debug;
use rayon::prelude::*;
use serde::Deserialize;
use validator::Validate;

use crate::config::Config;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::db;
use crate::exchanges::{Exchange, Exchanges};
use crate::forex;

use self::cache::Cache;
use self::custom_provider::{CustomProvider, CustomProviderConfig};
use self::fcsapi::{FcsApi, FcsApiConfig};
use self::finex::Finex;
use self::finnhub::{Finnhub, FinnhubConfig};
use self::moex::Moex;
use self::static_provider::{StaticProvider, StaticProviderConfig};
use self::tinkoff::{Tinkoff, TinkoffExchange};

#[derive(Clone)]
pub enum QuoteQuery {
    Forex(String),
    Stock(String, Vec<Exchange>),
}

enum QuoteRequest {
    Forex,
    Stock(Vec<Exchange>),
}

impl QuoteQuery {
    fn symbol(&self) -> &str {
        match self {
            QuoteQuery::Forex(ref pair) => pair,
            QuoteQuery::Stock(ref symbol, ..) => symbol,
        }
    }
}

#[derive(Deserialize, Default, Validate)]
#[serde(deny_unknown_fields)]
pub struct QuotesConfig {
    pub fcsapi: Option<FcsApiConfig>,
    pub finnhub: Option<FinnhubConfig>,
    #[validate]
    custom_provider: Option<CustomProviderConfig>,
    #[serde(rename="static")]
    static_provider: Option<StaticProviderConfig>,
}

pub struct Quotes {
    cache: Cache,
    providers: Vec<Arc<dyn QuotesProvider>>,
    batched_requests: RefCell<HashMap<String, QuoteRequest>>,
}

pub type QuotesRc = Rc<Quotes>;

impl Quotes {
    pub fn new(config: &Config, database: db::Connection) -> GenericResult<Quotes> {
        let mut providers = Vec::<Arc<dyn QuotesProvider>>::new();
        let mut has_custom_provider = false;

        let tinkoff = config.brokers.as_ref()
            .and_then(|brokers| brokers.tinkoff.as_ref())
            .and_then(|tinkoff| tinkoff.api.as_ref());

        // Prefer custom provider over the others
        if let Some(config) = config.quotes.custom_provider.as_ref() {
            providers.push(Arc::new(CustomProvider::new(config)));
            has_custom_provider = true;
        }

        // Static provider is used to complement and override default providers
        if let Some(config) = config.quotes.static_provider.as_ref() {
            providers.push(Arc::new(StaticProvider::new(config)));
            has_custom_provider = true;
        }

        // Prefer Tinkoff for forex (FCS API has too restrictive rate limits)
        if let Some(config) = tinkoff {
            providers.push(Arc::new(Tinkoff::new(config, TinkoffExchange::Currency)));
        }

        // Use FCS API for forex
        if let Some(config) = config.quotes.fcsapi.as_ref() {
            providers.push(Arc::new(FcsApi::new(config)))
        } else if !has_custom_provider {
            return Err!("FCS API access key is not set in the configuration file");
        }

        // Use Tinkoff for SPB stocks
        if let Some(config) = tinkoff {
            providers.push(Arc::new(Tinkoff::new(config, TinkoffExchange::Spb)));
        }

        // Use Finnhub for US stocks
        if let Some(config) = config.quotes.finnhub.as_ref() {
            providers.push(Arc::new(Finnhub::new(config)))
        } else if !has_custom_provider {
            return Err!("Finnhub token is not set in the configuration file");
        }

        // Prefer FinEx provider over MOEX until their funds are suspended
        providers.push(Arc::new(Finex::new("https://api.finex-etf.ru")));
        providers.push(Arc::new(Moex::new("https://iss.moex.com", "TQTF")));
        providers.push(Arc::new(Moex::new("https://iss.moex.com", "TQBR")));

        // As a best effort for unsupported exchanges provide a fallback to Tinkoff SPB/OTC stocks
        if let Some(config) = tinkoff {
            providers.push(Arc::new(Tinkoff::new(config, TinkoffExchange::Unknown)));
        }

        Ok(Quotes::new_with(Cache::new(database, config.cache_expire_time, true), providers))
    }

    fn new_with(cache: Cache, providers: Vec<Arc<dyn QuotesProvider>>) -> Quotes {
        Quotes {
            cache: cache,
            providers: providers,
            batched_requests: RefCell::new(HashMap::new()),
        }
    }

    pub fn batch(&self, query: QuoteQuery) -> GenericResult<Option<Cash>> {
        match query {
            QuoteQuery::Forex(symbol) => self.batch_forex(symbol),
            QuoteQuery::Stock(symbol, exchanges) => self.batch_stock(symbol, exchanges),
        }
    }

    pub fn batch_all<T>(&self, queries: T) -> EmptyResult
        where T: IntoIterator<Item=QuoteQuery>
    {
        for query in queries.into_iter() {
            self.batch(query)?;
        }
        Ok(())
    }

    pub fn execute(&self) -> EmptyResult {
        self.execute_query_plan(self.build_query_plan())
    }

    pub fn get(&self, query: QuoteQuery) -> GenericResult<Cash> {
        if let Some(price) = self.batch(query.clone())? {
            return Ok(price);
        }

        self.execute()?;

        Ok(self.cache.get(query.symbol())?.unwrap())
    }

    fn batch_forex(&self, mut symbol: String) -> GenericResult<Option<Cash>> {
        let (base, quote) = forex::parse_currency_pair(&symbol)?;

        if let Some(price) = self.cache.get(&symbol)? {
            return Ok(Some(price));
        }

        // Reverse pair quote sometimes slightly differs from `1 / pair`, but in some places we use
        // redundant currency conversions back and forth assuming that eventual result won't differ
        // more than rounding precision (for example in stock selling simulation when user specifies
        // base currency to calculate the performance in).
        //
        // To workaround the issue we make quotes consistent here.
        if base < quote {
            symbol = forex::get_currency_pair(quote, base)
        }

        match self.batched_requests.borrow_mut().entry(symbol) {
            Entry::Vacant(entry) => {
                entry.insert(QuoteRequest::Forex);
            },
            Entry::Occupied(entry) => match entry.get() {
                QuoteRequest::Forex => {},
                QuoteRequest::Stock(_) => unreachable!(),
            },
        }

        Ok(None)
    }

    fn batch_stock(&self, symbol: String, exchanges: Vec<Exchange>) -> GenericResult<Option<Cash>> {
        if forex::parse_currency_pair(&symbol).is_ok() {
            return Err!("Got {:?} stock which looks like a currency pair", symbol);
        }
        assert!(!exchanges.is_empty());

        if let Some(price) = self.cache.get(&symbol)? {
            return Ok(Some(price));
        }

        match self.batched_requests.borrow_mut().entry(symbol) {
            Entry::Vacant(entry) => {
                entry.insert(QuoteRequest::Stock(exchanges));
            },
            Entry::Occupied(mut entry) => match entry.get_mut() {
                QuoteRequest::Stock(prev_exchanges) => {
                    // Select most precise query
                    if exchanges.len() < prev_exchanges.len() {
                        entry.insert(QuoteRequest::Stock(exchanges));
                    }
                },
                QuoteRequest::Forex => unreachable!(),
            },
        }

        Ok(None)
    }

    fn build_query_plan(&self) -> HashMap<String, Vec<usize>> {
        let mut plan = HashMap::new();

        for (symbol, request) in self.batched_requests.borrow_mut().drain() {
            let mut providers = Vec::new();

            match request {
                QuoteRequest::Forex => {
                    for (index, provider) in self.providers.iter().enumerate() {
                        if provider.supports_forex() {
                            providers.push(index);
                        }
                    }
                },
                QuoteRequest::Stock(exchanges) => {
                    for exchange in self.pre_process_stock_exchanges(exchanges) {
                        for (index, provider) in self.providers.iter().enumerate() {
                            match provider.supports_stocks() {
                                SupportedExchange::Some(provider_exchange) => {
                                    if provider_exchange == exchange {
                                        providers.push(index);
                                    }
                                },
                                SupportedExchange::Any => {
                                    providers.push(index);
                                },
                                SupportedExchange::None => {},
                            }
                        }
                    }
                },
            }

            plan.insert(symbol, providers);
        }

        plan
    }

    fn has_stock_provider(&self, exchange: Exchange) -> bool {
        self.providers.iter().any(|provider| provider.supports_stocks() == SupportedExchange::Some(exchange))
    }

    fn pre_process_stock_exchanges(&self, mut exchanges: Vec<Exchange>) -> Vec<Exchange> {
        // Try to find OTC stocks on all known exchanges
        if exchanges.contains(&Exchange::Otc) {
            let mut new_exchanges = Exchanges::new_empty();

            for exchange in exchanges.into_iter().rev() {
                if exchange == Exchange::Otc {
                    new_exchanges.add_prioritized(Exchange::Moex);
                    new_exchanges.add_prioritized(Exchange::Spb);
                    new_exchanges.add_prioritized(Exchange::Other);
                    new_exchanges.add_prioritized(Exchange::Us);
                }
                new_exchanges.add_prioritized(exchange);
            }

            exchanges = new_exchanges.get_prioritized();
        }

        // Emulate SPB provider if we don't have it
        if exchanges.contains(&Exchange::Spb) && !self.has_stock_provider(Exchange::Spb) {
            let mut new_exchanges = Exchanges::new_empty();

            for exchange in exchanges.into_iter().rev() {
                if exchange == Exchange::Spb {
                    new_exchanges.add_prioritized(Exchange::Moex);
                    new_exchanges.add_prioritized(Exchange::Other);
                    new_exchanges.add_prioritized(Exchange::Us);
                } else {
                    new_exchanges.add_prioritized(exchange);
                }
            }

            exchanges = new_exchanges.get_prioritized();
        }

        exchanges
    }

    fn execute_query_plan(&self, mut plan: HashMap<String, Vec<usize>>) -> EmptyResult {
        let mut pass = 0;

        loop {
            let mut pass_plan: HashMap<usize, Vec<String>> = HashMap::new();

            for (symbol, providers) in plan.iter() {
                if let Some(&provider_id) = providers.get(pass) {
                    pass_plan.entry(provider_id).or_default().push(symbol.clone());
                }
            }

            if pass_plan.is_empty() {
                break;
            }

            let pass_plan: Vec<_> = pass_plan.into_iter().map(|(provider_id, symbols)| {
                (self.providers[provider_id].clone(), symbols)
            }).collect();

            for result in pass_plan.into_par_iter().map(|(provider, symbols)| -> GenericResult<(Arc<dyn QuotesProvider>, QuotesMap)> {
                debug!("Getting quotes from {} for the following symbols: {}...",
                       provider.name(), symbols.join(", "));

                let symbols: Vec<_> = symbols.iter().map(String::as_str).collect();
                let quotes = provider.get_quotes(&symbols).map_err(|e| format!(
                    "Failed to get quotes from {}: {}", provider.name(), e))?;

                Ok((provider, quotes))
            }).collect::<Vec<_>>() {
                let (provider, quotes) = result?;

                for (symbol, mut price) in quotes {
                    match forex::parse_currency_pair(&symbol) {
                        // Forex
                        Ok((base, quote)) => {
                            // Forex providers are allowed to return quotes for currency pairs only
                            // in one direction, so expect here that provider might return reverse
                            // pair instead of requested one.
                            //
                            // Plus see notes above about reverse pairs consistency with direct ones.
                            let reverse_pair = forex::get_currency_pair(quote, base);
                            let reverse_price = Cash::new(base, dec!(1) / price.amount);
                            self.cache.save(&reverse_pair, reverse_price)?;
                            plan.remove(&reverse_pair);
                        },

                        // Stocks
                        Err(_) => {
                            // Some providers return stock quotes with unnecessary very high precision,
                            // so add rounding here. But don't round Forex pairs since we always round
                            // conversion result + reverse pairs always need high precision.
                            if provider.high_precision() {
                                let rounded_price = price.round();
                                let round_precision = (price.amount - rounded_price.amount).abs() / price.amount;

                                if round_precision < dec!(0.0001) {
                                    price = rounded_price;
                                }
                            }
                        }
                    }

                    self.cache.save(&symbol, price)?;
                    plan.remove(&symbol);
                }
            }

            pass += 1;
        }

        if !plan.is_empty() {
            return Err!(
                "Unable to find quotes for following symbols: {}",
                plan.into_keys().join(", "));
        }

        Ok(())
    }
}

type QuotesMap = HashMap<String, Cash>;

#[derive(Clone, Copy, PartialEq)]
enum SupportedExchange {
    Any,
    None,
    Some(Exchange),
}

trait QuotesProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn supports_stocks(&self) -> SupportedExchange {SupportedExchange::None}
    fn supports_forex(&self) -> bool {false}
    fn high_precision(&self) -> bool {false}
    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::mutex_atomic)]
    fn cache() {
        struct AnyProvider {
            request_id: Mutex<usize>,
        }

        impl QuotesProvider for AnyProvider {
            fn name(&self) -> &'static str {
                "any-provider"
            }

            fn supports_stocks(&self) -> SupportedExchange {
                SupportedExchange::Any
            }

            fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
                let mut symbols = symbols.to_vec();
                symbols.sort_unstable();

                {
                    let mut request_id = self.request_id.lock().unwrap();
                    assert_eq!(*request_id, 0);
                    assert_eq!(&symbols, &["BND", "BNDX", "IWDA", "VTI"]);
                    *request_id += 1;
                }

                Ok(hashmap! {
                    s!("IWDA") => Cash::new("USD", dec!(79.76)),
                })
            }
        }

        struct FirstProvider {
            request_id: Mutex<usize>,
        }

        impl QuotesProvider for FirstProvider {
            fn name(&self) -> &'static str {
                "first-provider"
            }

            fn supports_stocks(&self) -> SupportedExchange {
                SupportedExchange::Some(Exchange::Us)
            }

            fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
                let mut symbols = symbols.to_vec();
                symbols.sort_unstable();

                {
                    let mut request_id = self.request_id.lock().unwrap();
                    assert_eq!(*request_id, 0);
                    assert_eq!(&symbols, &["BND", "BNDX", "VTI"]);
                    *request_id += 1;
                }

                Ok(hashmap! {
                    s!("BND") => Cash::new("USD", dec!(12.34)),
                    s!("VTI") => Cash::new("USD", dec!(56.78)),
                })
            }
        }

        struct SecondProvider {
            request_id: Mutex<usize>,
        }

        impl QuotesProvider for SecondProvider {
            fn name(&self) -> &'static str {
                "second-provider"
            }

            fn supports_stocks(&self) -> SupportedExchange {
                SupportedExchange::Some(Exchange::Us)
            }

            fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
                {
                    let mut request_id = self.request_id.lock().unwrap();
                    assert_eq!(*request_id, 0);
                    assert_eq!(symbols, ["BNDX"]);
                    *request_id += 1;
                }

                Ok(hashmap! {
                    s!("BNDX") => Cash::new("USD", dec!(90.12)),
                })
            }
        }

        struct OtherProvider {
        }

        impl QuotesProvider for OtherProvider {
            fn name(&self) -> &'static str {
                "other-provider"
            }

            fn supports_stocks(&self) -> SupportedExchange {
                SupportedExchange::Some(Exchange::Moex)
            }

            fn supports_forex(&self) -> bool {
                true
            }

            fn get_quotes(&self, _symbols: &[&str]) -> GenericResult<QuotesMap> {
                unreachable!()
            }
        }

        let (_database, cache) = Cache::new_temporary();
        let quotes = Quotes::new_with(cache, vec![
            Arc::new(AnyProvider {request_id: Mutex::new(0)}),
            Arc::new(FirstProvider {request_id: Mutex::new(0)}),
            Arc::new(OtherProvider {}),
            Arc::new(SecondProvider {request_id: Mutex::new(0)}),
        ]);

        let query = |symbol: &str| QuoteQuery::Stock(symbol.to_owned(), vec![Exchange::Us]);

        assert!(quotes.batch(query("VTI")).unwrap().is_none());
        assert!(quotes.batch(query("IWDA")).unwrap().is_none());
        assert!(quotes.batch(query("BNDX")).unwrap().is_none());
        assert_eq!(quotes.get(query("BND")).unwrap(), Cash::new("USD", dec!(12.34)));

        assert!(quotes.batch(query("VTI")).unwrap().is_some());
        assert!(quotes.batch(query("IWDA")).unwrap().is_some());
        assert!(quotes.batch(query("VXUS")).unwrap().is_none());
        assert_eq!(quotes.get(query("BND")).unwrap(), Cash::new("USD", dec!(12.34)));
        assert_eq!(quotes.get(query("VTI")).unwrap(), Cash::new("USD", dec!(56.78)));
        assert_eq!(quotes.get(query("IWDA")).unwrap(), Cash::new("USD", dec!(79.76)));
        assert_eq!(quotes.get(query("BNDX")).unwrap(), Cash::new("USD", dec!(90.12)));
    }
}