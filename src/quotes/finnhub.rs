use std::time::Duration;

#[cfg(test)] use indoc::indoc;
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
use crate::types::Decimal;

use super::{SupportedExchange, QuotesMap, QuotesProvider};
use super::common::{parallelize_quotes, send_request, is_outdated_unix_time};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinnhubConfig {
    #[serde(skip, default="FinnhubConfig::default_url")]
    url: String,
    token: String,
}

impl FinnhubConfig {
    fn default_url() -> String {
        s!("https://finnhub.io")
    }
}

pub struct Finnhub {
    url: String,
    token: String,

    client: Client,
    rate_limiter: RateLimiter,
}

impl Finnhub {
    pub fn new(config: &FinnhubConfig) -> Finnhub {
        Finnhub {
            url: config.url.clone(),
            token: config.token.clone(),

            client: Client::new(),
            rate_limiter: RateLimiter::new()
                .with_limit(60 / 2, Duration::from_secs(60))
                .with_limit(5, Duration::from_secs(1)),
        }
    }

    fn get_quote(&self, symbol: &str) -> GenericResult<Option<Cash>> {
        #[derive(Deserialize)]
        struct Quote {
            #[serde(rename = "t")]
            day_start_time: Option<i64>,

            #[serde(rename = "c")]
            current_price: Option<Decimal>,
        }

        let (time, price) = match self.query::<Quote>("quote", symbol)? {
            Some(Quote{
                day_start_time: Some(time),
                current_price: Some(price),
            }) if !price.is_zero() => (time, price),
            _ => return Ok(None),
        };

        if let Some(time) = is_outdated_unix_time(time, 1582295300)? {
            debug!("{symbol}: Got outdated quotes: {time}.");
            return Ok(None);
        }

        let price = util::validate_decimal(price, DecimalRestrictions::StrictlyPositive)
            .map_err(|_| format!("Got an invalid {symbol} price: {price:?}"))?;

        // Profile API has too expensive rate limit weight, so try to avoid using it
        let currency = if symbol.contains('.') {
            #[derive(Deserialize)]
            struct Profile {
                currency: String,
            }

            let profile = match self.query::<Profile>("stock/profile2", symbol)? {
                Some(profile) => profile,
                None => return Ok(None),
            };

            profile.currency
        } else {
            s!("USD")
        };

        Ok(Some(Cash::new(&currency, price)))
    }

    fn query<T: DeserializeOwned>(&self, method: &str, symbol: &str) -> GenericResult<Option<T>> {
        let url = Url::parse_with_params(&format!("{}/api/v1/{}", self.url, method), &[
            ("symbol", symbol),
            ("token", self.token.as_ref()),
        ])?;

        let get = |url| -> GenericResult<Option<T>> {
            self.rate_limiter.wait(&format!("request to {url}"));

            let reply = send_request(&self.client, url, None)?.text()?;
            if reply.trim() == "Symbol not supported" {
                return Ok(None);
            }

            Ok(serde_json::from_str(&reply)?)
        };

        Ok(get(&url).map_err(|e| format!("Failed to get quotes from {url}: {e}"))?)
    }
}

impl QuotesProvider for Finnhub {
    fn name(&self) -> String {
        s!("Finnhub")
    }

    fn supports_stocks(&self) -> SupportedExchange {
        SupportedExchange::Some(Exchange::Us)
    }

    fn high_precision(&self) -> bool {
        true
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        parallelize_quotes(symbols, |symbol| self.get_quote(symbol))
    }
}

#[cfg(test)]
mod tests {
    use mockito::{Server, Mock};
    use rstest::rstest;
    use super::*;

    #[rstest]
    fn quotes() {
        let mut server = Server::new();

        let client = Finnhub::new(&FinnhubConfig {
            url: server.url(),
            token: s!("mock")
        });

        let _bnd_profile_mock = mock(&mut server, "/api/v1/stock/profile2?symbol=BND&token=mock", indoc!(r#"
            {
                "country": "US",
                "currency": "USD",
                "exchange": "NASDAQ NMS - GLOBAL MARKET",
                "finnhubIndustry": "N/A",
                "ipo": "",
                "logo": "https://static.finnhub.io/logo/fad711b8-80e5-11ea-bacd-00000000092a.png",
                "marketCapitalization": 0,
                "name": "Vanguard Total Bond Market Index Fund",
                "phone": "",
                "shareOutstanding": 0,
                "ticker": "BND",
                "weburl": "http://www.vanguard.com/"
            }
        "#));
        let _bnd_quote_mock = mock(&mut server, "/api/v1/quote?symbol=BND&token=mock", indoc!(r#"
            {
                "c": 85.80000305175781,
                "h": 85.93000030517578,
                "l": 85.7300033569336,
                "o": 85.76000213623047,
                "pc": 85.58999633789062,
                "t": 1582295400
            }
        "#));

        let _outdated_profile_mock = mock(&mut server, "/api/v1/stock/profile2?symbol=AMZN&token=mock", indoc!(r#"
             {
                "country": "US",
                "currency": "USD",
                "exchange": "NASDAQ NMS - GLOBAL MARKET",
                "finnhubIndustry": "Retail",
                "ipo": "1997-05-01",
                "logo": "https://static.finnhub.io/logo/967bf7b0-80df-11ea-abb4-00000000092a.png",
                "marketCapitalization": 1220375,
                "name": "Amazon.com Inc",
                "phone": "12062661000",
                "shareOutstanding": 498.776032,
                "ticker": "AMZN",
                "weburl": "http://www.amazon.com/"
            }
       "#));
        let _outdated_quote_mock = mock(&mut server, "/api/v1/quote?symbol=AMZN&token=mock", indoc!(r#"
            {
                "c": 2095.969970703125,
                "h": 2144.550048828125,
                "l": 2088,
                "o": 2142.14990234375,
                "pc": 2153.10009765625,
                "t": 1582295300
            }
        "#));

        let _unknown_profile_mock = mock(&mut server, "/api/v1/stock/profile2?symbol=UNKNOWN&token=mock", "{}");
        let _unknown_quote_mock = mock(&mut server, "/api/v1/quote?symbol=UNKNOWN&token=mock", "{}");

        // Old response for unknown symbols
        let _unknown_old_1_profile_mock = mock(&mut server, "/api/v1/stock/profile2?symbol=UNKNOWN_OLD_1&token=mock", "{}");
        let _unknown_old_1_quote_mock = mock(&mut server, "/api/v1/quote?symbol=UNKNOWN_OLD_1&token=mock", indoc!(r#"
            {
                "c": 0,
                "h": 0,
                "l": 0,
                "o": 0,
                "pc": 0
            }
        "#));
        let _unknown_old_2_profile_mock = mock(&mut server, "/api/v1/stock/profile2?symbol=UNKNOWN_OLD_2&token=mock", "{}");
        let _unknown_old_2_quote_mock = mock(&mut server, "/api/v1/quote?symbol=UNKNOWN_OLD_2&token=mock", "Symbol not supported");

        let _fxrl_profile_mock = mock(&mut server, "/api/v1/stock/profile2?symbol=FXRL.ME&token=mock", indoc!(r#"
            {
                "country": "IE",
                "currency": "RUB",
                "exchange": "MOSCOW EXCHANGE",
                "finnhubIndustry": "N/A",
                "ipo": "",
                "logo": "",
                "marketCapitalization": 0,
                "name": "FinEx Russian RTS Equity UCITS ETF (USD)",
                "phone": "",
                "shareOutstanding": 0,
                "ticker": "FXRL.ME",
                "weburl": ""
            }
        "#));
        let _fxrl_quote_mock = mock(&mut server, "/api/v1/quote?symbol=FXRL.ME&token=mock", indoc!(r#"
            {
                "c": 2758.5,
                "h": 2796,
                "l": 2734,
                "o": 2796,
                "pc": 2764,
                "t": 1582295400
            }
        "#));

        assert_eq!(client.get_quotes(&[
            "BND", "AMZN", "UNKNOWN", "UNKNOWN_OLD_1", "UNKNOWN_OLD_2", "FXRL.ME",
        ]).unwrap(), hashmap! {
            s!("BND")     => Cash::new("USD", dec!(85.80000305175781)),
            s!("FXRL.ME") => Cash::new("RUB", dec!(2758.5)),
        });
    }

    fn mock(server: &mut Server, path: &str, data: &str) -> Mock {
        // All responses are always 200 OK, some of them are returned with application/json content
        // type, some - with text/plain even for JSON payload.
        server.mock("GET", path)
            .with_status(200)
            .with_body(data)
            .create()
    }
}