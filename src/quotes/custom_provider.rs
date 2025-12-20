#[cfg(test)] use indoc::indoc;
use reqwest::Url;
use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use validator::Validate;

use crate::core::GenericResult;
#[cfg(test)] use crate::currency::Cash;
use crate::forex;
use crate::http;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::{SupportedExchange, QuotesMap, QuotesProvider};
use super::common::parse_response;

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct CustomProviderConfig {
    #[validate(url)]
    url: String,
}

pub struct CustomProvider {
    url: String,
    client: Client,
}

impl CustomProvider {
    pub fn new(config: &CustomProviderConfig) -> CustomProvider {
        CustomProvider {
            url: config.url.clone(),
            client: Client::new(),
        }
    }
}

impl QuotesProvider for CustomProvider {
    fn name(&self) -> String {
        s!("custom quotes provider")
    }

    fn supports_stocks(&self) -> SupportedExchange {
        SupportedExchange::Any
    }

    fn supports_forex(&self) -> bool {
        true
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        let url = Url::parse_with_params(&format!("{}/v1/quotes", self.url), &[
            ("symbols", &symbols.join(",")),
        ])?;

        Ok(http::send_request(&self.client, &url, None).and_then(get_quotes).map_err(|e| format!(
            "Failed to get quotes from {url}: {e}"))?)
    }
}

fn get_quotes(response: Response) -> GenericResult<QuotesMap> {
    #[derive(Deserialize, Validate)]
    struct Response {
        #[validate(nested)]
        quotes: Vec<Quote>,
    }

    #[derive(Deserialize, Validate)]
    struct Quote {
        #[validate(length(min = 1))]
        symbol: String,
        price: Decimal,
        currency: Option<String>,
    }

    let response: Response = parse_response(&response.text()?)?;
    response.validate().map_err(|e| format!(
        "The server returned an invalid response: {e}"))?;

    let mut quotes = QuotesMap::new();

    for quote in response.quotes {
        let symbol = quote.symbol;

        let currency = match forex::parse_currency_pair(&symbol) {
            Ok((_base_currency, quote_currency)) => {
                if matches!(quote.currency, Some(currency) if currency != quote_currency) {
                    return Err!("Got an unexpected currency for {} quotes", symbol);
                }
                quote_currency
            },
            Err(_) => {
                let currency = quote.currency.as_ref().ok_or_else(|| format!(
                    "Got {symbol} quotes without currency"))?;

                if currency.len() != 3 || !currency.chars().all(|c| c.is_uppercase()) {
                    return Err!("Got an invalid currency: {:?}", currency);
                }

                currency
            },
        };

        let price = util::validate_named_cash(
            "price", currency, quote.price,
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

        let _quotes_mock = mock(&mut server, "/v1/quotes?symbols=USD%2FRUB%2CHKD%2FRUB%2CIWDA%2CUNKNOWN", indoc!(r#"
            {
                "quotes": [{
                    "symbol": "USD/RUB",
                    "price": "81.7900"
                }, {
                    "symbol": "HKD/RUB",
                    "price": "10.262",
                    "currency": "RUB"
                }, {
                    "symbol": "IWDA",
                    "price": "79.76",
                    "currency": "USD"
                }]
            }
        "#));

        let mut quotes = QuotesMap::new();
        quotes.insert(s!("USD/RUB"), Cash::new("RUB", dec!(81.7900)));
        quotes.insert(s!("HKD/RUB"), Cash::new("RUB", dec!(10.262)));
        quotes.insert(s!("IWDA"), Cash::new("USD", dec!(79.76)));

        assert_eq!(client.get_quotes(&["USD/RUB", "HKD/RUB", "IWDA", "UNKNOWN"]).unwrap(), quotes);
    }

    fn create_server() -> (ServerGuard, CustomProvider) {
        let server = Server::new();

        let client = CustomProvider::new(&CustomProviderConfig {
            url: server.url(),
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