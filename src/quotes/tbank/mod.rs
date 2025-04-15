use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use chrono::{LocalResult, TimeZone, Utc};
use itertools::Itertools;
use log::{Level, debug, log_enabled, trace};
use serde::Deserialize;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tonic::{Request, Status};
use tonic::service::{Interceptor, interceptor::InterceptedService};
use tonic::transport::{Channel, ClientTlsConfig};

#[allow(clippy::all)]
mod api {
    include!("tinkoff.public.invest.api.contract.v1.rs");
}

use api::{
    instruments_service_client::InstrumentsServiceClient, InstrumentsRequest, InstrumentStatus, RealExchange,
    market_data_service_client::MarketDataServiceClient, GetLastPricesRequest,
};

use crate::core::{GenericResult, EmptyResult};
use crate::exchanges::Exchange;
use crate::forex;
use crate::util::{self, DecimalRestrictions};
use crate::time::SystemTime;
use crate::types::Decimal;

use super::{SupportedExchange, QuotesMap, QuotesProvider};
use super::common::is_outdated_quote;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TbankApiConfig {
    #[serde(rename = "api_token")]
    token: String,
}

// T-Bank Invest API (https://tinkoff.github.io/investAPI/)
pub struct Tbank {
    token: String,
    exchange: TbankExchange,

    channel: Channel,
    runtime: Runtime,

    stocks: Mutex<HashMap<String, Vec<Stock>>>,
    currencies: Mutex<HashMap<(String, String), Currency>>,
}

impl Tbank {
    pub fn new(config: &TbankApiConfig, exchange: TbankExchange) -> GenericResult<Tbank> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all().build().unwrap();

        let channel = runtime.block_on(async {
            Channel::from_static("https://sandbox-invest-public-api.tinkoff.ru")
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(REQUEST_TIMEOUT)
                .tls_config(ClientTlsConfig::new().with_native_roots())
                .map(|endpoint| endpoint.connect_lazy())
        }).map_err(|e| format!("T-Bank client: {e}"))?;

        Ok(Tbank {
            token: config.token.clone(),
            exchange: exchange,

            channel: channel,
            runtime: runtime,

            stocks: Mutex::new(HashMap::new()),
            currencies: Mutex::new(HashMap::new()),
        })
    }

    fn instruments_client(&self) -> InstrumentsServiceClient<InterceptedService<Channel, ClientInterceptor>> {
        InstrumentsServiceClient::with_interceptor(self.channel.clone(), ClientInterceptor::new(&self.token))
    }

    fn market_data_client(&self) -> MarketDataServiceClient<InterceptedService<Channel, ClientInterceptor>> {
        MarketDataServiceClient::with_interceptor(self.channel.clone(), ClientInterceptor::new(&self.token))
    }

    async fn get_quotes_async(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        let mut instruments = HashMap::new();

        for &symbol in symbols {
            if let Ok((base, quote)) = forex::parse_currency_pair(symbol) {
                if let Some(currency) = self.get_currency(base, quote).await? {
                    match instruments.insert(currency.uid.clone(), Instrument::Currency(currency)) {
                        Some(Instrument::Stock(stock)) => {
                            return Err!(
                                "Got {} stock and {} currency pair which have the same instrument UID",
                                stock.symbol, symbol)
                        },
                        Some(Instrument::Currency(_)) | None => {},
                    }
                }
            } else if let Some(stock) = self.get_stock(symbol).await? {
                match instruments.insert(stock.uid.clone(), Instrument::Stock(stock)) {
                    Some(Instrument::Stock(stock)) => if stock.symbol != symbol {
                        return Err!("Got two stocks which have the same instrument UID: {} and {}",
                                    symbol, stock.symbol);
                    },
                    Some(Instrument::Currency(currency)) => {
                        return Err!(
                            "Got {} stock and {} currency pair which have the same instrument UID",
                            symbol, currency.symbol)
                    },
                    None => {},
                };
            }
        }

        let mut quotes = QuotesMap::new();
        if instruments.is_empty() {
            return Ok(quotes);
        }

        trace!(
            "Getting quotes for the following symbols from T-Bank: {}...",
            instruments.values().map(|instrument| match instrument {
                Instrument::Stock(stock) => stock.symbol.clone(),
                Instrument::Currency(currency) => forex::get_currency_pair(&currency.base, &currency.quote),
            }).sorted().join(", ")
        );

        let last_prices = self.market_data_client().get_last_prices(GetLastPricesRequest {
            instrument_id: instruments.keys().cloned().collect(),
            ..Default::default()
        }).await?.into_inner().last_prices;

        for last_price in last_prices {
            let instrument = instruments.remove(&last_price.instrument_uid).ok_or_else(|| format!(
                "Got quotes for an unexpected instrument UID: {:?}", last_price.instrument_uid))?;

            let (symbol, currency, denomination) = match instrument {
                Instrument::Stock(stock) => (stock.symbol, stock.currency, dec!(1)),
                Instrument::Currency(currency) => {
                    let symbol = forex::get_currency_pair(&currency.base, &currency.quote);
                    (symbol, currency.quote, currency.denomination)
                },
            };

            let (price, timestamp) = match (last_price.price, last_price.time) {
                (Some(price), Some(timestamp)) => (price, timestamp),
                _ => continue,
            };

            let time = match Utc.timestamp_opt(timestamp.seconds, timestamp.nanos as u32) {
                LocalResult::Single(time) => time,
                _ => return Err!("Got an invalid {} quote time: {:?}", symbol, timestamp)
            };

            if let Some(time) = is_outdated_quote(time, &SystemTime()) {
                debug!("{}: Got outdated quotes: {}.", symbol, time);
                continue;
            }

            let price = (Decimal::from(price.units) + Decimal::new(price.nano.into(), 9)) / denomination;

            let price = util::validate_named_cash(
                "price", &currency, price.normalize(),
                DecimalRestrictions::StrictlyPositive)?;

            quotes.insert(symbol, price);
        }

        Ok(quotes)
    }

    async fn get_currency(&self, base: &str, quote: &str) -> GenericResult<Option<Currency>> {
        let mut currencies = self.currencies.lock().await;

        if currencies.is_empty() {
            let instruments = self.instruments_client().currencies(InstrumentsRequest {
                ..Default::default()
            }).await.map_err(|e| format!(
                "Failed to get available currencies list: {}", e,
            ))?.into_inner().instruments;

            if instruments.is_empty() {
                return Err!("Got an empty list of available currencies");
            }

            trace!("Got the following currencies from T-Bank:");
            for currency in instruments {
                trace!("* {name} ({symbol})", name=currency.name, symbol=currency.ticker);

                let quote = currency.currency.to_uppercase();
                let (base, denomination) = match currency.nominal.as_ref() {
                    Some(nominal) if nominal.units > 0 && nominal.nano == 0 => {
                        (nominal.currency.to_uppercase(), Decimal::from(nominal.units))
                    },
                    _ => {
                        return Err!("Got {:?} currency pair with an invalid nominal info: {:?}",
                                    currency.ticker, currency.nominal);
                    },
                };

                currencies.insert((base.clone(), quote.clone()), Currency {
                    uid: currency.uid,
                    symbol: currency.ticker,
                    base: base,
                    quote: quote,
                    denomination: denomination,
                });
            }
        }

        Ok(currencies.get(&(base.to_owned(), quote.to_owned())).or_else(|| {
            currencies.get(&(quote.to_owned(), base.to_owned()))
        }).cloned())
    }

    async fn get_stock(&self, symbol: &str) -> GenericResult<Option<Stock>> {
        let mut stocks = self.stocks.lock().await;

        if stocks.is_empty() {
            let mut instruments = HashMap::new();

            self.get_all_shares(&mut instruments).await?;
            self.get_all_etfs(&mut instruments).await?;

            *stocks = instruments;
        }

        let found_stocks = match stocks.get(symbol) {
            Some(instruments) => instruments,
            None => return Ok(None),
        };

        if found_stocks.len() > 1 {
            let names = found_stocks.iter().map(|stock| {
                format!("{} ({})", stock.name, stock.isin)
            }).join(", ");
            return Err!("Got more than one stock for {:?} symbol: {}", symbol, names);
        }

        Ok(found_stocks.first().cloned())
    }

    async fn get_all_shares(&self, stocks: &mut HashMap<String, Vec<Stock>>) -> EmptyResult {
        let (name, status) = match self.exchange {
            TbankExchange::Spb => ("SPB stocks", InstrumentStatus::Base),
            TbankExchange::Unknown => ("other stocks", InstrumentStatus::All),
            TbankExchange::Currency => unreachable!(),
        };

        let mut trace = InstrumentTrace::new(name, false);

        #[allow(clippy::needless_update)]
        let instruments = self.instruments_client().shares(InstrumentsRequest {
            instrument_status: Some(status.into()),
            ..Default::default()
        }).await.map_err(|e| format!(
            "Failed to get available {} list: {}", name, e,
        ))?.into_inner().instruments;

        for stock in instruments {
            let real_exchange = stock.real_exchange();

            if !self.match_stock(real_exchange, &stock.exchange) {
                trace.skipped(real_exchange, stock.exchange, stock.ticker);
                continue;
            }

            stocks.entry(stock.ticker.clone()).or_default().push(Stock {
                uid: stock.uid,
                isin: stock.isin,
                symbol: stock.ticker.clone(),
                name: stock.name,
                currency: stock.currency.to_uppercase(),
            });

            trace.found(real_exchange, stock.exchange, stock.ticker);
        }

        trace.finish()
    }

    async fn get_all_etfs(&self, stocks: &mut HashMap<String, Vec<Stock>>) -> EmptyResult {
        let (name, status, may_be_empty) = match self.exchange {
            TbankExchange::Spb => ("SPB ETF", InstrumentStatus::Base, true),
            TbankExchange::Unknown => ("other ETF", InstrumentStatus::All, false),
            TbankExchange::Currency => unreachable!(),
        };

        // When SPB Exchange got under US sanctions, the list became empty, so we should allow it
        let mut trace = InstrumentTrace::new(name, may_be_empty);

        #[allow(clippy::needless_update)]
        let instruments = self.instruments_client().etfs(InstrumentsRequest {
            instrument_status: Some(status.into()),
            ..Default::default()
        }).await.map_err(|e| format!(
            "Failed to get available {} list: {}", name, e,
        ))?.into_inner().instruments;

        for stock in instruments {
            let real_exchange = stock.real_exchange();

            if !self.match_stock(real_exchange, &stock.exchange) {
                trace.skipped(real_exchange, stock.exchange, stock.ticker);
                continue;
            }

            stocks.entry(stock.ticker.clone()).or_default().push(Stock {
                uid: stock.uid,
                isin: stock.isin,
                symbol: stock.ticker.clone(),
                name: stock.name,
                currency: stock.currency.to_uppercase(),
            });

            trace.found(real_exchange, stock.exchange, stock.ticker);
        }

        trace.finish()
    }

    fn match_stock(&self, real_exchange: RealExchange, exchange: &str) -> bool {
        // Skipping some strange exchanges
        if matches!(exchange, "Issuance" | "moex_close" | "spb_close") {
            return false;
        }

        match self.exchange {
            TbankExchange::Spb => {
                real_exchange == RealExchange::Rts ||

                // SPB Hong Kong ETF (iShares Core MSCI Asia ex Japan ETF / 83010 for example) have unspecified real
                // exchange, so match them by exchange name
                exchange == "SPB_HK"
            },

            TbankExchange::Unknown => {
                // Some stocks from other exchanges are available as OTC, some - as SPB, so use all real exchanges
                // except MOEX
                real_exchange != RealExchange::Moex
            },

            TbankExchange::Currency => unreachable!(),
        }
    }
}

impl QuotesProvider for Tbank {
    fn name(&self) -> &'static str {
        "T-Bank"
    }

    fn supports_stocks(&self) -> SupportedExchange {
        match self.exchange {
            TbankExchange::Currency => SupportedExchange::None,
            TbankExchange::Spb => SupportedExchange::Some(Exchange::Spb),
            TbankExchange::Unknown => SupportedExchange::Some(Exchange::Other),
        }
    }

    fn supports_forex(&self) -> bool {
        matches!(self.exchange, TbankExchange::Currency)
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        self.runtime.block_on(self.get_quotes_async(symbols))
    }
}

#[derive(Clone, Copy)]
pub enum TbankExchange {
    Currency,
    Spb,
    Unknown, // Try to collect here instruments from exchanges that we don't support yet to use it as best effort fallback
}

enum Instrument {
    Stock(Stock),
    Currency(Currency),
}

#[derive(Clone)]
struct Stock {
    uid: String,
    isin: String,
    symbol: String,
    name: String,
    currency: String,
}

#[derive(Clone)]
struct Currency {
    uid: String,
    symbol: String,
    base: String,
    quote: String,
    denomination: Decimal,
}

struct ClientInterceptor {
    token: String,
}

impl ClientInterceptor {
    fn new(token: &str) -> ClientInterceptor {
        ClientInterceptor {
            token: token.to_owned(),
        }
    }
}

impl Interceptor for ClientInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let metadata = request.metadata_mut();

        metadata.insert("x-app-name", "KonishchevDmitry.investments".parse().map_err(|_|
            Status::invalid_argument("Invalid application name"))?);

        metadata.insert("authorization", format!("Bearer {}", self.token).parse().map_err(|_|
            Status::invalid_argument("Invalid token value"))?);

        request.set_timeout(REQUEST_TIMEOUT);
        Ok(request)
    }
}

struct InstrumentTrace {
    name: &'static str,
    count: usize,
    may_be_empty: bool,
    found_by_symbol: BTreeMap<String, Vec<(RealExchange, String)>>,
    found_by_exchange: BTreeMap<RealExchange, BTreeMap<String, BTreeSet<String>>>,
    skipped_by_exchange: BTreeMap<RealExchange, BTreeMap<String, BTreeSet<String>>>,
}

impl InstrumentTrace {
    fn new(name: &'static str, may_be_empty: bool) -> InstrumentTrace {
        trace!("Getting a list of available {} from T-Bank...", name);

        InstrumentTrace {
            name,
            count: 0,
            may_be_empty,
            found_by_symbol: BTreeMap::new(),
            found_by_exchange: BTreeMap::new(),
            skipped_by_exchange: BTreeMap::new(),
        }
    }

    fn found(&mut self, real_exchange: RealExchange, exchange: String, symbol: String) {
        self.count += 1;

        if log_enabled!(Level::Trace) {
            self.found_by_symbol.entry(symbol.clone()).or_default()
                .push((real_exchange, exchange.clone()));

            self.found_by_exchange.entry(real_exchange).or_default()
                .entry(exchange).or_default()
                .insert(symbol);
        }
    }

    fn skipped(&mut self, real_exchange: RealExchange, exchange: String, symbol: String) {
        if log_enabled!(Level::Trace) {
            self.skipped_by_exchange.entry(real_exchange).or_default()
                .entry(exchange).or_default()
                .insert(symbol);
        }
    }

    fn finish(self) -> EmptyResult {
        if log_enabled!(Level::Trace) {
            trace!("Got the following {} from T-Bank:", self.name);
            for (real_exchange, exchanges) in self.found_by_exchange {
                trace!("* {}:", real_exchange.as_str_name());
                for (exchange, symbols) in exchanges {
                    trace!("  * {}: {}", exchange, symbols.iter().join(", "));
                }
            }

            if !self.skipped_by_exchange.is_empty() {
                trace!("Skipped non-{} from T-Bank:", self.name);
                for (real_exchange, exchanges) in self.skipped_by_exchange {
                    trace!("* {}:", real_exchange.as_str_name());
                    for (exchange, symbols) in exchanges {
                        trace!("  * {}: {}", exchange, symbols.iter().join(", "));
                    }
                }
            }

            let mut has_duplicates = false;
            for (symbol, exchanges) in self.found_by_symbol {
                if exchanges.len() == 1 {
                    continue;
                }

                if !has_duplicates {
                    trace!("Duplicated {} from T-Bank:", self.name);
                    has_duplicates = true;
                }

                trace!("* {}: {}", symbol, exchanges.iter().map(|(real_exchange, exchange)| {
                    format!("{}/{}", real_exchange.as_str_name(), exchange)
                }).join(", "));
            }
        }

        if !self.may_be_empty && self.count == 0 {
            return Err!("Got an empty list of available {}", self.name);
        }

        Ok(())
    }
}