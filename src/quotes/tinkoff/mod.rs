use std::collections::HashMap;
use std::time::Duration;

use chrono::{LocalResult, TimeZone, Utc};
use itertools::Itertools;
use log::{debug, trace};
use serde::Deserialize;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tonic::{Request, Status};
use tonic::service::{Interceptor, interceptor::InterceptedService};
use tonic::transport::Channel;

#[allow(clippy::all)]
mod api {
    include!("tinkoff.public.invest.api.contract.v1.rs");
}

use api::{
    instruments_service_client::InstrumentsServiceClient, InstrumentsRequest, RealExchange,
    market_data_service_client::MarketDataServiceClient, GetLastPricesRequest,
};

use crate::core::GenericResult;
use crate::exchanges::Exchange;
use crate::util::{self, DecimalRestrictions};
use crate::time::SystemTime;
use crate::types::Decimal;

use super::{QuotesMap, QuotesProvider, get_currency_pair, parse_currency_pair};
use super::common::is_outdated_quote;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TinkoffApiConfig {
    #[serde(rename = "api_token")]
    token: String,
}

// Tinkoff Investments API (https://tinkoff.github.io/investAPI/)
pub struct Tinkoff {
    token: String,

    channel: Channel,
    runtime: Runtime,

    stocks: Mutex<HashMap<String, Vec<Stock>>>,
    currencies: Mutex<HashMap<(String, String), Currency>>,
}

impl Tinkoff {
    pub fn new(config: &TinkoffApiConfig) -> Tinkoff {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all().build().unwrap();

        let channel = runtime.block_on(async {
            Channel::from_static("https://sandbox-invest-public-api.tinkoff.ru")
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(REQUEST_TIMEOUT)
                .connect_lazy()
        });

        Tinkoff {
            token: config.token.clone(),

            channel: channel,
            runtime: runtime,

            stocks: Mutex::new(HashMap::new()),
            currencies: Mutex::new(HashMap::new()),
        }
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
            if let Ok((base, quote)) = parse_currency_pair(symbol) {
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
            "Getting quotes for the following symbols from Tinkoff: {}...",
            instruments.values().map(|instrument| match instrument {
                Instrument::Stock(stock) => stock.symbol.clone(),
                Instrument::Currency(currency) => get_currency_pair(&currency.base, &currency.quote)
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
                    let symbol = get_currency_pair(&currency.base, &currency.quote);
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

            trace!("Got the following currencies from Tinkoff:");
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
            trace!("Getting a list of available stocks from Tinkoff...");

            let instruments = self.instruments_client().shares(InstrumentsRequest {
                ..Default::default()
            }).await.map_err(|e| format!(
                "Failed to get available stocks list: {}", e,
            ))?.into_inner().instruments;

            trace!("Got the following stocks from Tinkoff:");
            for stock in instruments {
                if stock.real_exchange() != RealExchange::Rts {
                    continue
                }

                trace!("* {name} ({symbol})", name=stock.name, symbol=stock.ticker);
                stocks.entry(stock.ticker.clone()).or_default().push(Stock {
                    uid: stock.uid,
                    isin: stock.isin,
                    symbol: stock.ticker,
                    name: stock.name,
                    currency: stock.currency.to_uppercase(),
                })
            }

            if stocks.is_empty() {
                return Err!("Got an empty list of available stocks");
            }
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
}

impl QuotesProvider for Tinkoff {
    fn name(&self) -> &'static str {
        "Tinkoff"
    }

    fn supports_stocks(&self) -> Option<Exchange> {
        Some(Exchange::Spb)
    }

    fn supports_forex(&self) -> bool {
        true
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        self.runtime.block_on(self.get_quotes_async(symbols))
    }
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