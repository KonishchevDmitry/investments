use std::collections::HashMap;

#[cfg(test)] use indoc::indoc;
use log::error;
#[cfg(test)] use mockito::{self, Mock, mock};
use reqwest::Url;
use reqwest::blocking::{Client, Response};
use serde::Deserialize;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::exchanges::Exchange;
use crate::time;
use crate::util::{self, DecimalRestrictions};

use super::{QuotesMap, QuotesProvider};
use super::common::{send_request, is_outdated_time};

pub struct AlphaVantage {
    api_key: String,
    client: Client,
}

impl AlphaVantage {
    // At some time has become too restrictive in API limits - only 5 RPM and deprecated batch
    // quotes API which makes it unusable for stocks now, but maybe will be useful for forex quotes
    // in the future.
    #[allow(dead_code)]
    pub fn new(token: &str) -> AlphaVantage {
        AlphaVantage {
            api_key: token.to_owned(),
            client: Client::new(),
        }
    }
}

impl QuotesProvider for AlphaVantage {
    fn name(&self) -> &'static str {
        "Alpha Vantage"
    }

    fn supports_stocks(&self) -> Option<Exchange> {
        Some(Exchange::Us)
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        #[cfg(not(test))] let base_url = "https://www.alphavantage.co";
        #[cfg(test)] let base_url = mockito::server_url();

        let url = Url::parse_with_params(&format!("{}/query", base_url), &[
            ("function", "BATCH_STOCK_QUOTES"),
            ("symbols", symbols.join(",").as_ref()),
            ("apikey", self.api_key.as_ref()),
        ])?;

        Ok(send_request(&self.client, &url).and_then(|response| {
            Ok(parse_quotes(response).map_err(|e| format!(
                "Quotes info parsing error: {}", e))?)
        }).map_err(|e| format!("Failed to get quotes from {}: {}", url, e))?)
    }
}

fn parse_quotes(response: Response) -> GenericResult<HashMap<String, Cash>> {
    #[derive(Deserialize)]
    struct Response {
        #[serde(rename = "Meta Data")]
        metadata: Metadata,

        #[serde(rename = "Stock Quotes")]
        quotes: Vec<Quote>,
    }

    #[derive(Deserialize)]
    struct Metadata {
        #[serde(rename = "3. Time Zone")]
        timezone: String,
    }

    #[derive(Deserialize)]
    struct Quote {
        #[serde(rename = "1. symbol")]
        symbol: String,

        #[serde(rename = "2. price")]
        price: String,

        #[serde(rename = "4. timestamp")]
        time: String,
    }

    let response: Response = response.json()?;
    let timezone = time::parse_timezone(&response.metadata.timezone)?;

    let mut quotes = HashMap::new();
    let mut outdated = Vec::new();

    for quote in response.quotes {
        let time = time::parse_tz_date_time(&quote.time, "%Y-%m-%d %H:%M:%S", timezone, true)?;

        // A special case for quotes that are returned by API but don't updated and have zero price
        if time.timestamp() == 0 {
            continue;
        }

        if is_outdated_time(time, date_time!(2018, 10, 31, 20, 0, 0)).is_some() {
            outdated.push(quote.symbol);
            continue;
        }

        let price = util::parse_decimal(&quote.price, DecimalRestrictions::StrictlyPositive)
            .map_err(|_| format!("Invalid price: {:?}", quote.price))?.normalize();

        quotes.insert(quote.symbol, Cash::new("USD", price));
    };

    if !outdated.is_empty() {
        error!("Got outdated quotes for the following symbols: {}.", outdated.join(", "));
    }

    Ok(quotes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_quotes() {
        let _mock = mock_response(
            "/query?function=BATCH_STOCK_QUOTES&symbols=BND%2CBNDX&apikey=mock",
            indoc!(r#"
                {
                    "Meta Data": {
                        "1. Information": "Batch Stock Market Quotes",
                        "2. Notes": "IEX Real-Time",
                        "3. Time Zone": "US/Eastern"
                    },
                    "Stock Quotes": []
                }
            "#)
        );

        let client = AlphaVantage::new("mock");
        assert_eq!(client.get_quotes(&["BND", "BNDX"]).unwrap(), HashMap::new());
    }

    #[test]
    fn quotes() {
        let _mock = mock_response(
            "/query?function=BATCH_STOCK_QUOTES&symbols=BND%2CBNDX%2COUTDATED%2CINVALID&apikey=mock",
            indoc!(r#"
                {
                    "Meta Data": {
                        "1. Information": "Batch Stock Market Quotes",
                        "2. Notes": "IEX Real-Time",
                        "3. Time Zone": "US/Eastern"
                    },
                    "Stock Quotes": [
                        {
                            "1. symbol": "BND",
                            "2. price": "77.8650",
                            "3. volume": "6044682",
                            "4. timestamp": "2018-10-31 16:00:05"
                        },
                        {
                            "1. symbol": "BNDX",
                            "2. price": "54.5450",
                            "3. volume": "977142",
                            "4. timestamp": "2018-10-31 16:00:08"
                        },
                        {
                            "1. symbol": "OUTDATED",
                            "2. price": "138.5000",
                            "3. volume": "4034572",
                            "4. timestamp": "2018-10-31 16:00:00"
                        }
                    ]
                }
            "#)
        );

        let client = AlphaVantage::new("mock");

        let mut quotes = HashMap::new();
        quotes.insert(s!("BND"), Cash::new("USD", dec!(77.8650)));
        quotes.insert(s!("BNDX"), Cash::new("USD", dec!(54.5450)));
        assert_eq!(client.get_quotes(&["BND", "BNDX", "OUTDATED", "INVALID"]).unwrap(), quotes);
    }

    fn mock_response(path: &str, data: &str) -> Mock {
        mock("GET", path)
            .with_status(200)
            .with_header("Content-Type", "application/json")
            .with_body(data)
            .create()
    }
}