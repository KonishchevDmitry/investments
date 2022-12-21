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

use super::{QuotesMap, QuotesProvider};
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

    instruments: Mutex<HashMap<String, Vec<Instrument>>>
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

        let client = Tinkoff {
            token: config.token.clone(),

            channel: channel,
            runtime: runtime,

            instruments: Mutex::new(HashMap::new()),
        };
        // FIXME(konishchev): Remove it when will be implemented
        // client.get_quotes(&["700", "1810"]).unwrap();
        // unreachable!();

        client
    }

    fn instruments_client(&self) -> InstrumentsServiceClient<InterceptedService<Channel, ClientInterceptor>> {
        InstrumentsServiceClient::with_interceptor(self.channel.clone(), ClientInterceptor::new(&self.token))
    }

    fn market_data_client(&self) -> MarketDataServiceClient<InterceptedService<Channel, ClientInterceptor>> {
        MarketDataServiceClient::with_interceptor(self.channel.clone(), ClientInterceptor::new(&self.token))
    }

    async fn get_instrument(&self, symbol: &str) -> GenericResult<Option<Instrument>> {
        let mut instruments = self.instruments.lock().await;

        if instruments.is_empty() {
            trace!("Getting a list of available stocks from Tinkoff...");

            let stocks = self.instruments_client().shares(InstrumentsRequest {
                ..Default::default()
            }).await.map_err(|e| format!(
                "Failed to get available stocks list: {}", e,
            ))?.into_inner().instruments;

            trace!("Got the following stocks from Tinkoff:");
            for stock in stocks {
                if stock.real_exchange() != RealExchange::Rts {
                    continue
                }

                trace!("* {name} ({symbol})", name=stock.name, symbol=stock.ticker);
                instruments.entry(stock.ticker.clone()).or_default().push(Instrument {
                    uid: stock.uid,
                    isin: stock.isin,
                    symbol: stock.ticker,
                    name: stock.name,
                    currency: stock.currency.to_uppercase(),
                })
            }

            if instruments.is_empty() {
                return Err!("Got an empty list of available stocks");
            }
        }

        let matched_instruments = match instruments.get(symbol) {
            Some(instruments) => instruments,
            None => return Ok(None),
        };

        if matched_instruments.len() > 1 {
            let names = matched_instruments.iter().map(|instrument| {
                format!("{} ({})", instrument.name, instrument.isin)
            }).join(", ");
            return Err!("Got more than one instrument for {:?} symbol: {}", symbol, names);
        }

        Ok(matched_instruments.first().cloned())
    }
}

impl QuotesProvider for Tinkoff {
    fn name(&self) -> &'static str {
        "Tinkoff"
    }

    fn supports_stocks(&self) -> Option<Exchange> {
        Some(Exchange::Spb)
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        self.runtime.block_on(async {
            let mut instruments = HashMap::new();
            let mut request = GetLastPricesRequest {
                ..Default::default()
            };

            for symbol in symbols {
                if let Some(instrument) = self.get_instrument(symbol).await? {
                    request.instrument_id.push(instrument.uid.clone());
                    if let Some(other) = instruments.insert(instrument.uid.clone(), instrument) {
                        return Err!("Got a duplicated instrument with {:?} UID: {} ({})",
                            other.uid, other.name, other.symbol);
                    }
                }
            }

            trace!("Getting quotes for the following symbols from Tinkoff: {}...",
                instruments.values().map(|instrument| &instrument.symbol).sorted().join(", "));

            let prices = self.market_data_client().get_last_prices(request).await?
                .into_inner().last_prices;

            let mut quotes = QuotesMap::new();

            for price in prices {
                let instrument = instruments.remove(&price.instrument_uid).ok_or_else(|| format!(
                    "Got quotes for an unexpected instrument: {:?}", price.instrument_uid))?;

                let (price, timestamp) = match (price.price, price.time) {
                    (Some(price), Some(timestamp)) => (price, timestamp),
                    _ => continue,
                };

                let time = match Utc.timestamp_opt(timestamp.seconds, timestamp.nanos as u32) {
                    LocalResult::Single(time) => time,
                    _ => return Err!("Got an invalid quote time: {:?}", timestamp)
                };

                if let Some(time) = is_outdated_quote(time, &SystemTime()) {
                    debug!("{}: Got outdated quotes: {}.", instrument.symbol, time);
                    continue;
                }

                let price = Decimal::from(price.units) + Decimal::new(price.nano as i64, 9);

                let price = util::validate_named_cash(
                    "price", &instrument.currency, price.normalize(),
                    DecimalRestrictions::StrictlyPositive)?;

                quotes.insert(instrument.symbol, price);
            }

            Ok(quotes)
        })
    }
}

#[derive(Clone)]
struct Instrument {
    uid: String,
    isin: String,
    symbol: String,
    name: String,
    currency: String,
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