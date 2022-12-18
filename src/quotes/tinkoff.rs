use std::borrow::Borrow;
use std::ops::{Add, Sub};
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
#[cfg(test)] use indoc::indoc;
use itertools::Itertools;
use log::debug;
use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::exchanges::Exchange;
use crate::rate_limiter::RateLimiter;
use crate::util::{self, DecimalRestrictions};
use crate::time::TimeProvider;
use crate::types::Decimal;

use super::{QuotesMap, QuotesProvider};
use super::common::{parallelize_quotes, send_request, is_outdated_quote};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TinkoffApiConfig {
    token: String,
}

// Tinkoff Investments API (https://tinkoff.github.io/invest-openapi/)
pub struct Tinkoff {
    token: String,
    client: Client,
    rate_limiter: RateLimiter,
    time_provider: Box<dyn TimeProvider>,
}

impl Tinkoff {
    pub fn new(config: &TinkoffApiConfig, time_provider: Box<dyn TimeProvider>) -> Tinkoff {
        Tinkoff {
            token: config.token.clone(),
            client: Client::new(),
            rate_limiter: RateLimiter::new().with_limit(100, Duration::from_secs(60)),
            time_provider: time_provider,
        }
    }

    fn get_quote(&self, symbol: &str) -> GenericResult<Option<Cash>> {
        use chrono::Duration;

        let instrument = match self.get_instrument(symbol)? {
            Some(instrument) => instrument,
            None => return Ok(None),
        };

        for (period, candle_interval, candle_interval_name) in [
            (Duration::days(1), Duration::minutes(1), "1min"),
            (Duration::days(7), Duration::hours(1),   "hour"),
        ] {
            let quote = self.get_quote_from_candles(
                &instrument.figi, period, candle_interval, candle_interval_name)?;

            let (price, time) = match quote {
                Some((price, time)) => (price, time),
                None => continue,
            };

            if let Some(time) = is_outdated_quote(time, self.time_provider.as_ref()) {
                debug!("{}: Got outdated quotes: {}.", symbol, time);
                return Ok(None);
            }

            return Ok(Some(Cash::new(&instrument.currency, price)))
        }

        Ok(None)
    }

    fn get_instrument(&self, symbol: &str) -> GenericResult<Option<Instrument>> {
        #[derive(Deserialize)]
        struct Result {
            payload: Payload,
        }

        #[derive(Deserialize)]
        struct Payload {
            instruments: Vec<Instrument>,
        }

        let result: Result = self.query("/market/search/by-ticker", &[("ticker", symbol)])?;
        let instruments = result.payload.instruments;

        if instruments.len() > 1 {
            let names = instruments.iter().map(|instrument| {
                format!("{} ({})", instrument.name, instrument.isin)
            }).join(", ");

            return Err!("Got more than one instrument for {:?} symbol: {}", symbol, names);
        }

        Ok(instruments.into_iter().next())
    }

    fn get_quote_from_candles(
        &self, figi: &str, period: chrono::Duration,
        candle_interval: chrono::Duration, candle_interval_name: &str
    ) -> GenericResult<Option<(Decimal, DateTime<Utc>)>> {
        #[derive(Deserialize)]
        struct Result {
            payload: Payload,
        }

        #[derive(Deserialize)]
        struct Payload {
            candles: Vec<Candle>,
        }

        #[derive(Deserialize)]
        struct Candle {
            time: String,
            interval: String,
            #[serde(rename = "c")]
            close_price: Decimal,
        }

        const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";
        let now = self.time_provider.now().naive_utc();

        let from = now.sub(period).format(TIME_FORMAT).to_string();
        let to = now.format(TIME_FORMAT).to_string();

        let result: Result = self.query("/market/candles", &[
            ("figi", figi),
            ("from", from.as_str()),
            ("to", to.as_str()),
            ("interval", candle_interval_name),
        ])?;

        let candle = match result.payload.candles.last() {
            Some(candle) => candle,
            None => return Ok(None),
        };

        let time = {
            let start = Utc.datetime_from_str(&candle.time, TIME_FORMAT).map_err(|_| format!(
                "Got an invalid time: {:?}", candle.time))?;

            if candle.interval != candle_interval_name {
                return Err!("Got an unexpected candle interval: {}", candle.interval);
            }

            start.add(candle_interval)
        };

        let price = util::validate_decimal(candle.close_price, DecimalRestrictions::StrictlyPositive)
            .map_err(|_| format!("Got an invalid price: {:?}", candle.close_price))?;

        Ok(Some((price, time)))
    }

    fn query<R, P, K, V>(&self, method: &str, params: P) -> GenericResult<R>
        where
            R: DeserializeOwned,
            P: IntoIterator,
            P::Item: Borrow<(K, V)>,
            K: AsRef<str>,
            V: AsRef<str>,
    {
        #[cfg(not(test))] let base_url = "https://api-invest.tinkoff.ru/openapi/sandbox";
        #[cfg(test)] let base_url = mockito::server_url();

        let url = Url::parse_with_params(&format!("{}{}", base_url, method), params)?;

        let get = |url| -> GenericResult<R> {
            self.rate_limiter.wait(&format!("request to {}", url));
            let reply = send_request(&self.client, url, Some(&self.token))?.text()?;
            Ok(serde_json::from_str(&reply)?)
        };

        Ok(get(&url).map_err(|e| format!("Failed to get quotes from {}: {}", url, e))?)
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
        parallelize_quotes(symbols, |symbol| self.get_quote(symbol))
    }
}

#[derive(Deserialize)]
struct Instrument {
    figi: String,
    isin: String,
    name: String,
    currency: String,
}

#[cfg(test)]
mod tests {
    use mockito::{Mock, mock};
    use rstest::{rstest, fixture};

    use crate::time::FakeTime;
    use super::*;

    #[fixture]
    fn client() -> Tinkoff {
        let now = Utc.from_utc_datetime(&date_time!(2022, 12, 16, 11, 21, 58));
        let config = TinkoffApiConfig {
            token: s!("token-mock")
        };
        Tinkoff::new(&config, Box::new(FakeTime::new(now)))
    }

    #[rstest]
    fn quotes(client: Tinkoff) {
        let _tencent_info = mock_response("/market/search/by-ticker?ticker=700", indoc!(r#"
            {
                "trackingId": "27b020602524ac6b",
                "payload": {
                    "instruments": [
                        {
                            "figi": "BBG000BJ35N5",
                            "ticker": "700",
                            "isin": "KYG875721634",
                            "minPriceIncrement": 0.2,
                            "lot": 1,
                            "currency": "HKD",
                            "name": "Tencent Holdings",
                            "type": "Stock"
                        }
                    ],
                    "total": 1
                },
                "status": "Ok"
            }
        "#));

        let _xiaomi_info = mock_response("/market/search/by-ticker?ticker=1810", indoc!(r#"
            {
                "trackingId": "2f695cddfb4dd518",
                "payload": {
                    "instruments": [
                        {
                            "figi": "BBG00KVTBY91",
                            "ticker": "1810",
                            "isin": "KYG9830T1067",
                            "minPriceIncrement": 0.01,
                            "lot": 100,
                            "currency": "HKD",
                            "name": "Xiaomi",
                            "type": "Stock"
                        }
                    ],
                    "total": 1
                },
                "status": "Ok"
            }
        "#));

        let _apple_info = mock_response("/market/search/by-ticker?ticker=AAPL", indoc!(r#"
            {
                "trackingId": "728ec0c5279c30f0",
                "payload": {
                    "instruments": [
                        {
                            "figi": "BBG000B9XRY4",
                            "ticker": "AAPL",
                            "isin": "US0378331005",
                            "minPriceIncrement": 0.01,
                            "lot": 1,
                            "currency": "USD",
                            "name": "Apple",
                            "type": "Stock"
                        }
                    ],
                    "total": 1
                },
                "status": "Ok"
            }
        "#));

        let _unknown_info = mock_response("/market/search/by-ticker?ticker=UNKNOWN", indoc!(r#"
            {
                "trackingId": "cfe61104843bce63",
                "payload": {
                    "instruments": [],
                    "total": 0
                },
                "status": "Ok"
            }
        "#));

        let _tencent_empty_minute_candles = mock_response("/market/candles?figi=BBG000BJ35N5&from=2022-12-15T11%3A21%3A58Z&to=2022-12-16T11%3A21%3A58Z&interval=1min", indoc!(r#"
            {
                "trackingId": "8b36e5c9fbc49852",
                "payload": {
                    "candles": [],
                    "interval": "1min",
                    "figi": "BBG000BJ35N5"
                },
                "status": "Ok"
            }
        "#));

        let _tencent_hour_candles = mock_response("/market/candles?figi=BBG000BJ35N5&from=2022-12-09T11%3A21%3A58Z&to=2022-12-16T11%3A21%3A58Z&interval=hour", indoc!(r#"
            {
                "trackingId": "26c529a83fca0028",
                "payload": {
                    "candles": [
                        {
                            "o": 317,
                            "c": 316.6,
                            "h": 317,
                            "l": 316.6,
                            "v": 159,
                            "time": "2022-12-16T10:00:00Z",
                            "interval": "hour",
                            "figi": "BBG000BJ35N5"
                        },
                        {
                            "o": 316.5,
                            "c": 318.6,
                            "h": 318.6,
                            "l": 316,
                            "v": 135,
                            "time": "2022-12-16T11:00:00Z",
                            "interval": "hour",
                            "figi": "BBG000BJ35N5"
                        }
                    ],
                    "interval": "hour",
                    "figi": "BBG000BJ35N5"
                },
                "status": "Ok"
            }
        "#));

        let _xiaomi_empty_minute_candles = mock_response("/market/candles?figi=BBG00KVTBY91&from=2022-12-15T11%3A21%3A58Z&to=2022-12-16T11%3A21%3A58Z&interval=1min", indoc!(r#"
            {
                "trackingId": "8b36e5c9fbc49852",
                "payload": {
                    "candles": [],
                    "interval": "1min",
                    "figi": "BBG00KVTBY91"
                },
                "status": "Ok"
            }
        "#));

        let _xiaomi_empty_hour_candles = mock_response("/market/candles?figi=BBG00KVTBY91&from=2022-12-09T11%3A21%3A58Z&to=2022-12-16T11%3A21%3A58Z&interval=hour", indoc!(r#"
            {
                "trackingId": "26c529a83fca0028",
                "payload": {
                    "candles": [],
                    "interval": "hour",
                    "figi": "BBG00KVTBY91"
                },
                "status": "Ok"
            }
        "#));

        let _apple_minute_candles = mock_response("/market/candles?figi=BBG000B9XRY4&from=2022-12-15T11%3A21%3A58Z&to=2022-12-16T11%3A21%3A58Z&interval=1min", indoc!(r#"
            {
                "trackingId": "fe3c5d35bbd10bda",
                "payload": {
                    "candles": [
                        {
                            "o": 135.8,
                            "c": 135.8,
                            "h": 135.8,
                            "l": 135.8,
                            "v": 11,
                            "time": "2022-12-16T11:20:00Z",
                            "interval": "1min",
                            "figi": "BBG000B9XRY4"
                        },
                        {
                            "o": 135.75,
                            "c": 135.76,
                            "h": 135.76,
                            "l": 135.75,
                            "v": 3,
                            "time": "2022-12-16T11:21:00Z",
                            "interval": "1min",
                            "figi": "BBG000B9XRY4"
                        }
                    ],
                    "interval": "1min",
                    "figi": "BBG000B9XRY4"
                },
                "status": "Ok"
            }
        "#));

        assert_eq!(client.get_quotes(&["700", "1810", "UNKNOWN", "AAPL"]).unwrap(), hashmap! {
            s!("700")  => Cash::new("HKD", dec!(318.6)),
            s!("AAPL") => Cash::new("USD", dec!(135.76)),
        });
    }

    fn mock_response(path: &str, data: &str) -> Mock {
        mock("GET", path).match_header("Authorization", "Bearer token-mock")
            .with_status(200)
            .with_header("Content-Type", "application/json")
            .with_body(data)
            .create()
    }
}