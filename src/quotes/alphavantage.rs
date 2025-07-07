use std::collections::HashMap;

#[cfg(test)] use indoc::indoc;
use log::error;
use reqwest::Url;
use reqwest::blocking::{Client, Response};
use serde::Deserialize;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::time;
use crate::util::{self, DecimalRestrictions};

use super::{SupportedExchange, QuotesMap, QuotesProvider};
use super::common::{send_request, is_outdated_time};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaVantageConfig {
    #[serde(skip, default = "AlphaVantageConfig::default_url")]
    pub url: String,
    pub api_key: String,
}

impl AlphaVantageConfig {
    fn default_url() -> String {
        s!("https://www.alphavantage.co")
    }
}

pub struct AlphaVantage {
    url: String,
    api_key: String,
    client: Client,
}

impl AlphaVantage {
    pub fn new(config: &AlphaVantageConfig) -> AlphaVantage {
        AlphaVantage {
            url: config.url.clone(),
            api_key: config.api_key.clone(),
            client: Client::new(),
        }
    }

    pub fn find_symbol(&self, symbol: &str) -> GenericResult<HashMap<String, String>> {
        let url = Url::parse_with_params(&format!("{}/query", self.url), &[
            ("function", "SYMBOL_SEARCH"),
            ("keywords", symbol),
            ("apikey", self.api_key.as_ref()),
        ])?;

        Ok(send_request(&self.client, &url, None).and_then(|response| {
            parse_symbol_search(symbol, response)
        }).map_err(|e| format!("Failed to lookup {symbol} via {url}: {e}"))?)
    }
}

impl QuotesProvider for AlphaVantage {
    fn name(&self) -> &'static str {
        "Alpha Vantage"
    }

    // At some time it has became too restrictive in API limits - only 5 RPM + 25 RPD and deprecated batch quotes API
    // which makes it fully unusable.
    fn supports_stocks(&self) -> SupportedExchange {
        SupportedExchange::None
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        let url = Url::parse_with_params(&format!("{}/query", self.url), &[
            ("function", "BATCH_STOCK_QUOTES"),
            ("symbols", symbols.join(",").as_ref()),
            ("apikey", self.api_key.as_ref()),
        ])?;

        Ok(send_request(&self.client, &url, None).and_then(|response| {
            Ok(parse_quotes(response).map_err(|e| format!(
                "Quotes info parsing error: {e}"))?)
        }).map_err(|e| format!("Failed to get quotes from {url}: {e}"))?)
    }
}

fn parse_symbol_search(symbol: &str, response: Response) -> GenericResult<HashMap<String, String>> {
    #[derive(Deserialize)]
    struct Response {
        #[serde(rename = "bestMatches")]
        results: Vec<Match>,
    }

    #[derive(Deserialize)]
    struct Match {
        #[serde(rename = "1. symbol")]
        symbol: String,
        #[serde(rename = "8. currency")]
        currency: String,
    }

    let mut symbols = HashMap::new();
    let response: Response = response.json()?;

    for result in response.results {
        if result.symbol == symbol || result.symbol.strip_prefix(symbol)
            .and_then(|suffix| suffix.strip_prefix('.'))
            .map(|suffix| suffix.len() == 3)
            .unwrap_or_default() {
            symbols.insert(result.symbol, result.currency);
        }
    };

    Ok(symbols)
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
    use mockito::{Server, ServerGuard, Mock};
    use super::*;

    #[test]
    fn find_symbol_none() {
        let (mut server, client) = create_server();

        let _mock = mock(&mut server, "/query?function=SYMBOL_SEARCH&keywords=SSAC&apikey=mock", indoc!(r#"
            {
                "bestMatches": []
            }
        "#));

        assert_eq!(client.find_symbol("SSAC").unwrap(), HashMap::new());
    }

    #[test]
    fn find_symbol_fuzzy() {
        let (mut server, client) = create_server();

        let _mock = mock(&mut server, "/query?function=SYMBOL_SEARCH&keywords=IGLA&apikey=mock", indoc!(r#"
            {
                "bestMatches": [
                    {
                        "1. symbol": "IGLAX",
                        "2. name": "VOYA GLOBAL REAL ESTATE FUND CLASS A",
                        "3. type": "Mutual Fund",
                        "4. region": "United States",
                        "5. marketOpen": "09:30",
                        "6. marketClose": "16:00",
                        "7. timezone": "UTC-04",
                        "8. currency": "USD",
                        "9. matchScore": "0.8889"
                    },
                    {
                        "1. symbol": "IGLA.LON",
                        "2. name": "iShares Global Govt Bond UCITS Acc",
                        "3. type": "ETF",
                        "4. region": "United Kingdom",
                        "5. marketOpen": "08:00",
                        "6. marketClose": "16:30",
                        "7. timezone": "UTC+01",
                        "8. currency": "USD",
                        "9. matchScore": "0.8000"
                    }
                ]
            }
        "#));

        assert_eq!(client.find_symbol("IGLA").unwrap(), hashmap! {
            s!("IGLA.LON") => s!("USD"),
        });
    }

    #[test]
    fn find_symbol_multiple() {
        let (mut server, client) = create_server();

        let _mock = mock(&mut server, "/query?function=SYMBOL_SEARCH&keywords=SSAC&apikey=mock", indoc!(r#"
            {
                "bestMatches": [
                    {
                        "1. symbol": "SSAC.LON",
                        "2. name": "iShares MSCI ACWI UCITS ETF USD (Acc) GBP",
                        "3. type": "ETF",
                        "4. region": "United Kingdom",
                        "5. marketOpen": "08:00",
                        "6. marketClose": "16:30",
                        "7. timezone": "UTC+01",
                        "8. currency": "GBX",
                        "9. matchScore": "0.8000"
                    },
                    {
                        "1. symbol": "SSAC.AMS",
                        "2. name": "iShares MSCI ACWI UCITS ETF USD (Acc) EUR",
                        "3. type": "ETF",
                        "4. region": "Amsterdam",
                        "5. marketOpen": "09:00",
                        "6. marketClose": "17:40",
                        "7. timezone": "UTC+01",
                        "8. currency": "EUR",
                        "9. matchScore": "0.7273"
                    },
                    {
                        "1. symbol": "SSACD.PAR",
                        "2. name": "Euronext S Credit Agricole 070322 GR Decr 1.05",
                        "3. type": "Equity",
                        "4. region": "Paris",
                        "5. marketOpen": "09:00",
                        "6. marketClose": "17:30",
                        "7. timezone": "UTC+02",
                        "8. currency": "EUR",
                        "9. matchScore": "0.6667"
                    }
                ]
            }
        "#));

        assert_eq!(client.find_symbol("SSAC").unwrap(), hashmap! {
            s!("SSAC.LON") => s!("GBX"),
            s!("SSAC.AMS") => s!("EUR"),
        });
    }

    #[test]
    fn no_quotes() {
        let (mut server, client) = create_server();

        let _mock = mock(&mut server, "/query?function=BATCH_STOCK_QUOTES&symbols=BND%2CBNDX&apikey=mock", indoc!(r#"
            {
                "Meta Data": {
                    "1. Information": "Batch Stock Market Quotes",
                    "2. Notes": "IEX Real-Time",
                    "3. Time Zone": "US/Eastern"
                },
                "Stock Quotes": []
            }
        "#));

        assert_eq!(client.get_quotes(&["BND", "BNDX"]).unwrap(), HashMap::new());
    }

    #[test]
    fn quotes() {
        let (mut server, client) = create_server();

        let _mock = mock(&mut server, "/query?function=BATCH_STOCK_QUOTES&symbols=BND%2CBNDX%2COUTDATED%2CINVALID&apikey=mock", indoc!(r#"
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
        "#));

        let mut quotes = HashMap::new();
        quotes.insert(s!("BND"), Cash::new("USD", dec!(77.8650)));
        quotes.insert(s!("BNDX"), Cash::new("USD", dec!(54.5450)));
        assert_eq!(client.get_quotes(&["BND", "BNDX", "OUTDATED", "INVALID"]).unwrap(), quotes);
    }

    fn create_server() -> (ServerGuard, AlphaVantage) {
        let server = Server::new();

        let client = AlphaVantage::new(&AlphaVantageConfig {
            url: server.url(),
            api_key: s!("mock")
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