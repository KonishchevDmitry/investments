mod api;
mod auth;
mod trace;

use std::collections::HashMap;
use std::time::Duration;

use chrono::{Local, Months, TimeDelta};
use itertools::Itertools;
use log::{debug, trace};
use serde::Deserialize;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Channel, ClientTlsConfig};

use api::{
    InstrumentsRequest, InstrumentStatus, RealExchange, GetLastPricesRequest, GetCandlesRequest, CandleInterval,
    CandleSource, HistoricCandle, Quotation,
    instruments_service_client::InstrumentsServiceClient,
    market_data_service_client::MarketDataServiceClient,
};

use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::exchanges::Exchange;
use crate::{forex, formatting};
use crate::proto;
use crate::time::{self, Date, TzDateTime, SystemTime, TimeZone, Period};
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::{SupportedExchange, QuotesProvider, QuotesMap, HistoricalQuotes};
use super::common::is_outdated_quote;

use self::auth::ClientInterceptor;
use self::trace::InstrumentTrace;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TbankApiConfig {
    #[serde(rename = "api_token")]
    token: String,
}

#[derive(Clone, Copy)]
pub enum TbankExchange {
    Currency,
    Moex,
    Spb,
    Unknown, // Try to collect here instruments from exchanges that we don't support yet to use it as best effort fallback
}

// T-Bank Invest API (https://developer.tbank.ru/invest/api)
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
        let interceptor = ClientInterceptor::new(&self.token, REQUEST_TIMEOUT);
        InstrumentsServiceClient::with_interceptor(self.channel.clone(), interceptor)
    }

    fn market_data_client(&self) -> MarketDataServiceClient<InterceptedService<Channel, ClientInterceptor>> {
        let interceptor = ClientInterceptor::new(&self.token, REQUEST_TIMEOUT);
        MarketDataServiceClient::with_interceptor(self.channel.clone(), interceptor)
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

            let time = proto::parse_timestamp(timestamp, Local).ok_or_else(|| format!(
                "Got an invalid {symbol} quote time: {timestamp:?}"))?;

            if let Some(time) = is_outdated_quote(time, &SystemTime()) {
                debug!("{}: Got outdated quotes: {}.", symbol, time);
                continue;
            }

            let price = parse_price(price)? / denomination;
            quotes.insert(symbol, Cash::new(&currency, price));
        }

        Ok(quotes)
    }

    async fn get_historical_quotes_async(&self, symbol: &str, period: Period) -> GenericResult<Option<HistoricalQuotes>> {
        let Some(stock) = self.get_stock(symbol).await? else {
            return Ok(None);
        };

        let time_zone = match self.exchange {
            TbankExchange::Moex | TbankExchange::Spb => time::tz_to_fixed(chrono_tz::Europe::Moscow),
            TbankExchange::Unknown => time::tz_to_fixed(Local),
            TbankExchange::Currency => unreachable!(),
        };

        let (Some(from), Some(to)) = (
            time_zone.from_local_datetime(&period.first_date().into()).latest(),
            period.last_date().and_hms_opt(23, 59, 59).and_then(|time| time_zone.from_local_datetime(&time).latest())
        ) else {
            return Err!("Invalid period: {period}");
        };

        trace!("Getting historical quotes for {symbol} ({period}) from T-Bank...");

        let mut request_from = from;
        let mut quotes: HashMap<Date, Vec<Decimal>> = HashMap::new();

        loop {
            let interval = CandleInterval::Hour;
            let request_to = limit_historical_request_range(request_from, to, interval)?;

            trace!(
                "Requesting {symbol} ({} - {}) from T-Bank...",
                formatting::format_date(request_from.with_timezone(&Local).naive_local()),
                formatting::format_date(request_to.with_timezone(&Local).naive_local()),
            );

            let candles = self.market_data_client().get_candles(GetCandlesRequest {
                instrument_id: Some(stock.uid.clone()),
                candle_source_type: Some(CandleSource::Exchange.into()),

                from: Some(proto::new_timestamp(request_from)),
                to: Some(proto::new_timestamp(request_to)), // inclusive
                interval: interval.into(),

                ..Default::default()
            }).await?.into_inner().candles;

            if candles.is_empty() {
                trace!("Got an empty candles response.");
            } else {
                let debug_time = |candle: Option<&HistoricCandle>| {
                    candle.and_then(|candle| candle.time)
                        .and_then(|time| proto::parse_timestamp(time, Local))
                        .map(|time| formatting::format_date(time.naive_local()))
                        .unwrap_or_default()
                };
                trace!("Got candles for {} - {}.", debug_time(candles.first()), debug_time(candles.last()));
            }

            for candle in &candles {
                let (Some(time), Some(open), Some(close)) = (
                    candle.time.and_then(|time| proto::parse_timestamp(time, time_zone)),
                    candle.open, candle.close
                ) else {
                    return Err!("Got an invalid candle: {candle:?}");
                };

                let date = time.date_naive();
                let open = parse_price(open)?;
                let close = parse_price(close)?;

                let price = (open + close) / Decimal::from(2);
                quotes.entry(date).or_default().push(price);
            }

            if request_to >= to {
                return Ok(Some(aggregate_historical_quotes(&stock.currency, quotes)))
            }

            request_from = request_to.checked_add_signed(TimeDelta::seconds(1)).unwrap();
        }
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
            TbankExchange::Moex => ("MOEX stocks", InstrumentStatus::Base),
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
            TbankExchange::Moex => ("MOEX ETF", InstrumentStatus::Base, true),
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
            TbankExchange::Moex => real_exchange == RealExchange::Moex,

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

    fn supports_forex(&self) -> bool {
        matches!(self.exchange, TbankExchange::Currency)
    }

    fn supports_stocks(&self) -> SupportedExchange {
        match self.exchange {
            TbankExchange::Currency => SupportedExchange::None,
            TbankExchange::Moex => SupportedExchange::Some(Exchange::Moex),
            TbankExchange::Spb => SupportedExchange::Some(Exchange::Spb),
            TbankExchange::Unknown => SupportedExchange::Some(Exchange::Other),
        }
    }

    fn supports_historical_stocks(&self) -> SupportedExchange {
        self.supports_stocks()
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        self.runtime.block_on(self.get_quotes_async(symbols))
    }

    fn get_historical_quotes(&self, symbol: &str, perod: Period) -> GenericResult<Option<HistoricalQuotes>> {
        self.runtime.block_on(self.get_historical_quotes_async(symbol, perod))
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

fn parse_price(quote: Quotation) -> GenericResult<Decimal> {
    let price = Decimal::from(quote.units) + Decimal::new(quote.nano.into(), 9);
    util::validate_named_decimal("price", price.normalize(), DecimalRestrictions::StrictlyPositive)
}

// To fit into API limits – https://developer.tbank.ru/invest/intro/intro/load_history
fn limit_historical_request_range<Tz: TimeZone>(
    from: TzDateTime<Tz>, to: TzDateTime<Tz>, interval: CandleInterval
) -> GenericResult<TzDateTime<Tz>> {
    let max_months = match interval {
        CandleInterval::Hour => 3,
        CandleInterval::Day => 6 * 12,
        _ => return Err!("Unsupported candle interval: {:?}", interval),
    };

    let max_to = from.clone().checked_add_months(Months::new(max_months)).ok_or_else(|| format!(
        "Invalid request period: {} - {}",
        formatting::format_date(from.naive_local()),
        formatting::format_date(to.naive_local())
    ))?;

    Ok(std::cmp::min(to, max_to))
}

fn aggregate_historical_quotes(currency: &str, quotes: HashMap<Date, Vec<Decimal>>) -> HistoricalQuotes {
    quotes.into_iter().map(|(date, prices)| {
        let price = prices.iter().copied().sum::<Decimal>() / Decimal::from(prices.len());
        (date, Cash::new(currency, price).normalize())
    }).collect()
}