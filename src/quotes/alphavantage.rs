use std::collections::HashMap;

#[cfg(not(test))] use chrono;
#[cfg(test)] use chrono::NaiveDate;
#[cfg(test)] use indoc::indoc;
use chrono::{DateTime, TimeZone};
use chrono_tz::Tz;
use log::error;
#[cfg(test)] use mockito::{self, Mock, mock};
use reqwest::{Client, Url, Response};
use serde::Deserialize;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::util::{self, DecimalRestrictions};

use super::{QuotesMap, QuotesProvider};

pub struct AlphaVantage {
    api_key: String,
}

impl AlphaVantage {
    pub fn new(token: &str) -> AlphaVantage {
        AlphaVantage {
            api_key: token.to_owned(),
        }
    }
}

impl QuotesProvider for AlphaVantage {
    fn name(&self) -> &'static str {
        "Alpha Vantage"
    }

    fn get_quotes(&self, symbols: &[String]) -> GenericResult<QuotesMap> {
        #[cfg(not(test))] let base_url = "https://www.alphavantage.co";
        #[cfg(test)] let base_url = mockito::server_url();

        let url = Url::parse_with_params(&format!("{}/query", base_url), &[
            ("function", "BATCH_STOCK_QUOTES"),
            ("symbols", symbols.join(",").as_ref()),
            ("apikey", self.api_key.as_ref()),
        ])?;

        let get = |url| -> GenericResult<HashMap<String, Cash>> {
            let mut response = Client::new().get(url).send()?;
            if !response.status().is_success() {
                return Err!("The server returned an error: {}", response.status());
            }

            Ok(parse_quotes(&mut response).map_err(|e| format!(
                "Quotes info parsing error: {}", e))?)
        };

        Ok(get(url.as_str()).map_err(|e| format!(
            "Failed to get quotes from {}: {}", url, e))?)
    }
}

fn parse_quotes(response: &mut Response) -> GenericResult<HashMap<String, Cash>> {
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
    let timezone: Tz = response.metadata.timezone.parse().map_err(|_| format!(
        "Invalid time zone: {:?}", response.metadata.timezone))?;

    let mut quotes = HashMap::new();
    let mut outdated = Vec::new();

    for quote in response.quotes {
        let date_time = timezone.datetime_from_str(&quote.time, "%Y-%m-%d %H:%M:%S").map_err(|_| format!(
            "Invalid time: {:?}", quote.time))?;

        // A special case for quotes that are returned by API but don't updated and have zero price
        if date_time.timestamp() == 0 {
            continue;
        }

        if (date_time.naive_utc() - util::utc_now()).num_hours() > 0 {
            return Err!("Invalid time: {}. It's from future", quote.time);
        }

        if is_outdated(date_time) {
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

#[cfg(not(test))]
fn is_outdated<T: TimeZone>(date_time: DateTime<T>) -> bool {
    (util::utc_now() - date_time.naive_utc()).num_days() >= 5
}

#[cfg(test)]
fn is_outdated<T: TimeZone>(date_time: DateTime<T>) -> bool {
    date_time.naive_utc() <= NaiveDate::from_ymd(2018, 10, 31).and_hms(16 + 4, 0, 0)
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
        assert_eq!(client.get_quotes(&[s!("BND"), s!("BNDX")]).unwrap(), HashMap::new());
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

        assert_eq!(client.get_quotes(&[
            s!("BND"), s!("BNDX"), s!("OUTDATED"), s!("INVALID")
        ]).unwrap(), quotes);
    }

    fn mock_response(path: &str, data: &str) -> Mock {
        mock("GET", path)
            .with_status(200)
            .with_header("Content-Type", "application/json")
            .with_body(data)
            .create()
    }
}