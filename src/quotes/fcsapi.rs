use std::time::Duration;

#[cfg(test)] use indoc::indoc;
use log::debug;
use reqwest::Url;
use reqwest::blocking::{Client, Response};
use serde::Deserialize;

use crate::core::GenericResult;
#[cfg(test)] use crate::currency::Cash;
use crate::forex;
use crate::rate_limiter::RateLimiter;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::{SupportedExchange, QuotesMap, QuotesProvider};
use super::common::{send_request, parse_response, is_outdated_unix_time};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FcsApiConfig {
    #[serde(skip, default = "FcsApiConfig::default_url")]
    url: String,
    access_key: String,
}

impl FcsApiConfig {
    fn default_url() -> String {
        s!("https://fcsapi.com")
    }
}

pub struct FcsApi {
    url: String,
    access_key: String,

    client: Client,
    rate_limiter: RateLimiter,
}

impl FcsApi {
    pub fn new(config: &FcsApiConfig) -> FcsApi {
        FcsApi {
            url: config.url.clone(),
            access_key: config.access_key.clone(),

            client: Client::new(),
            rate_limiter: RateLimiter::new().with_quota(Duration::from_secs(30), 2),
        }
    }
}

impl QuotesProvider for FcsApi {
    fn name(&self) -> &'static str {
        "FCS API"
    }

    // Stocks are actually supported, but we use it only for Forex quotes due to small API rate limits
    fn supports_stocks(&self) -> SupportedExchange {
        SupportedExchange::None
    }

    fn supports_forex(&self) -> bool {
        true
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        let url = Url::parse_with_params(&format!("{}/api-v3/forex/latest", self.url), &[
            ("symbol", &symbols.join(",")),
            ("access_key", &self.access_key),
        ])?;

        self.rate_limiter.wait(&format!("request to {url}"));
        Ok(send_request(&self.client, &url, None).and_then(get_quotes).map_err(|e| format!(
            "Failed to get quotes from {url}: {e}"))?)
    }
}

fn get_quotes(response: Response) -> GenericResult<QuotesMap> {
    #[derive(Deserialize)]
    struct Response {
        status: bool,
        msg: String,
        #[serde(default, rename = "response")]
        quotes: Vec<Quote>,
    }

    #[derive(Deserialize)]
    struct Quote {
        #[serde(rename = "s")]
        symbol: String,
        #[serde(rename = "c")]
        price: Decimal,
        #[serde(rename = "t")]
        time: String,
    }

    let response: Response = parse_response(&response.text()?)?;
    if !response.status {
        return Err!("Server returned an error: {}", response.msg.trim_end_matches('.'));
    }

    let mut quotes = QuotesMap::new();

    for quote in response.quotes {
        let symbol = quote.symbol;
        let time: i64 = quote.time.parse().map_err(|_| format!(
            "Got an invalid UNIX timestamp: {:?}", quote.time))?;

        if let Some(time) = is_outdated_unix_time(time, 1650259200)? {
            debug!("{symbol}: Got outdated quotes: {time}.");
            continue
        }

        let (_base_currency, quote_currency) = forex::parse_currency_pair(&symbol)?;
        let price = util::validate_named_cash(
            "price", quote_currency, quote.price,
            DecimalRestrictions::StrictlyPositive)?;

        quotes.insert(symbol, price);
    }

    Ok(quotes)
}

#[cfg(test)]
mod tests {
    use mockito::{Server, ServerGuard, Mock};
    use rstest::rstest;
    use super::*;

    #[rstest]
    fn quotes() {
        let (mut server, client) = create_server();

        let _quotes_mock = mock(&mut server, "/api-v3/forex/latest?symbol=USD%2FRUB%2CUSD%2FEUR%2COUTDATED%2CUNKNOWN&access_key=mock", indoc!(r#"
            {
                "status": true,
                "code": 200,
                "msg": "Successfully",
                "response": [
                    {
                        "id": "1815",
                        "o": "0.92514",
                        "h": "0.92694",
                        "l": "0.9242",
                        "c": "0.92650",
                        "ch": "+0.00136",
                        "cp": "+0.15%",
                        "t": "1650260218",
                        "s": "USD/EUR",
                        "tm": "2022-04-18 05:36:58"
                    },
                    {
                        "id": "1872",
                        "o": "78.253",
                        "h": "84.5000",
                        "l": "82.0000",
                        "c": "82.4055",
                        "ch": "+4.15250",
                        "cp": "+5.31%",
                        "t": "1650259213",
                        "s": "USD/RUB",
                        "tm": "2022-04-18 05:20:13"
                    },
                    {
                        "id": "1817",
                        "o": "0.76562",
                        "h": "0.76898",
                        "l": "0.76540",
                        "c": "0.76810",
                        "ch": "+0.00248",
                        "cp": "+0.32%",
                        "t": "1650259200",
                        "s": "OUTDATED",
                        "tm": "2022-04-18 05:20:00"
                    }
                ],
                "info": {
                    "server_time": "2022-04-18 05:40:00 UTC",
                    "credit_count": 1,
                    "_t": "2022-04-18 05:40:00 UTC"
                }
            }
        "#));

        let mut quotes = QuotesMap::new();
        quotes.insert(s!("USD/RUB"), Cash::new("RUB", dec!(82.4055)));
        quotes.insert(s!("USD/EUR"), Cash::new("EUR", dec!(0.92650)));
        assert_eq!(client.get_quotes(&["USD/RUB", "USD/EUR", "OUTDATED", "UNKNOWN"]).unwrap(), quotes);
    }

    #[rstest]
    fn invalid_access_key() {
        let (mut server, client) = create_server();

        let _invalid_access_key_mock = mock(&mut server, "/api-v3/forex/latest?symbol=USD%2FRUB&access_key=mock", indoc!(r#"
            {
                "status": false,
                "code": 101,
                "msg": "You have not supplied a valid API Access Key.",
                "info": {
                    "credit_count": 0
                }
            }
        "#));

        let err = client.get_quotes(&["USD/RUB"]).expect_err("Invalid token error is expected");
        assert!(err.to_string().ends_with(": You have not supplied a valid API Access Key"));
    }

    fn create_server() -> (ServerGuard, FcsApi) {
        let server = Server::new();

        let client = FcsApi::new(&FcsApiConfig {
            url: server.url(),
            access_key: s!("mock")
        });

        (server, client)
    }

    fn mock(server: &mut Server, path: &str, data: &str) -> Mock {
        server.mock("GET", path)
            .with_status(200)
            .with_header("Content-Type", "application/json")
            .with_body(data)
            .create()
    }
}