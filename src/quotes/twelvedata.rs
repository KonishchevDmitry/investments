use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, TimeZone, Utc};
#[cfg(test)] use chrono::NaiveDate;
#[cfg(test)] use indoc::indoc;
use log::{debug, trace};
#[cfg(test)] use mockito::{self, Mock, mock};
use rayon::prelude::*;
use reqwest::Url;
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json;

use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::util::{self, DecimalRestrictions};
use crate::types::Decimal;

use super::{QuotesMap, QuotesProvider, parse_currency_pair};

pub struct TwelveData {
    token: String,
    client: Client,
}

impl TwelveData {
    pub fn new(token: &str) -> TwelveData {
        TwelveData {
            token: token.to_owned(),
            client: Client::new(),
        }
    }

    fn get_quote(&self, symbol: &str) -> GenericResult<Option<Cash>> {
        #[cfg(not(test))] let base_url = "https://api.twelvedata.com";
        #[cfg(test)] let base_url = mockito::server_url();

        let url = Url::parse_with_params(&format!("{}/time_series", base_url), &[
            ("symbol", symbol),
            ("interval", "1min"),
            ("outputsize", "1"),
            ("timezone", "UTC"),
            ("apikey", self.token.as_ref()),
        ])?;

        let get = |url| {
            trace!("Sending request to {}...", url);
            let response = self.client.get(url).send()?;
            trace!("Got response from {}.", url);

            if !response.status().is_success() {
                return Err!("Server returned an error: {}", response.status());
            }

            get_quote(symbol, response)
        };

        Ok(get(url.as_str()).map_err(|e| format!(
            "Failed to get quotes from {}: {}", url, e))?)
    }
}

impl QuotesProvider for TwelveData {
    fn name(&self) -> &'static str {
        "Twelve Data"
    }

    // FIXME: Waiting for ETF support
    fn supports_stocks(&self) -> bool {
        false
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        let quotes = Mutex::new(HashMap::new());

        if let Some(error) = symbols.par_iter().map(|&symbol| -> EmptyResult {
            if let Some(price) = self.get_quote(symbol)? {
                let mut quotes = quotes.lock().unwrap();
                quotes.insert(symbol.to_owned(), price);
            }
            Ok(())
        }).find_map_any(|result| match result {
            Err(error) => Some(error),
            Ok(()) => None,
        }) {
            return Err(error);
        }

        Ok(quotes.into_inner().unwrap())
    }
}

fn get_quote(symbol: &str, response: Response) -> GenericResult<Option<Cash>> {
    #[derive(Deserialize)]
    struct GenericResponse {
        status: String,
    }

    #[derive(Deserialize)]
    struct ErrorResponse {
        message: String,
    }

    #[derive(Deserialize)]
    struct QuoteResponse {
        meta: Meta,
        values: Vec<Value>,
    }

    #[derive(Deserialize)]
    struct Meta {
        currency: Option<String>,
    }

    #[derive(Deserialize)]
    struct Value {
        datetime: String,
        close: Decimal,
    }

    let response = response.text()?;

    if parse_response::<GenericResponse>(&response)?.status != "ok" {
        let error: ErrorResponse = parse_response(&response)?;
        debug!("{}: Server returned an error: {}.", symbol, error.message);
        return Ok(None)
    }

    let quote: QuoteResponse = parse_response(&response)?;

    let currency = if let Ok((_base_currency, quote_currency)) = parse_currency_pair(symbol) {
        if let Some(currency) = quote.meta.currency {
            if currency != quote_currency {
                return Err!(
                    "Got an unexpected currency for {} forex pair: {}", symbol, currency);
            }
        }

        quote_currency
    } else {
        quote.meta.currency.as_ref().ok_or_else(||
            "Got an unexpected response from server: missing quote currency")?.as_str()
    };

    let value = match quote.values.first() {
        Some(value) => value,
        None => return Ok(None),
    };

    let time = util::parse_tz_date_time(&value.datetime, "%Y-%m-%d %H:%M:%S", Utc, true)?;
    if is_outdated(time) {
        debug!("{}: Got outdated quotes: {}.", symbol, time);
        return Ok(None);
    }

    let price = util::validate_decimal(
        value.close, DecimalRestrictions::StrictlyPositive).map_err(|_| format!(
        "Invalid price: {:?}", value.close))?;

    Ok(Some(Cash::new(currency, price)))
}

fn parse_response<T: DeserializeOwned>(response: &str) -> GenericResult<T> {
    Ok(serde_json::from_str(&response).map_err(|e| format!("Got an unexpected response: {}", e))?)
}

#[cfg(not(test))]
fn is_outdated<T: TimeZone>(time: DateTime<T>) -> bool {
    super::is_outdated_quote(time)
}

#[cfg(test)]
fn is_outdated<T: TimeZone>(time: DateTime<T>) -> bool {
    time.naive_utc() <= NaiveDate::from_ymd(2020, 1, 31).and_hms(20, 58, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes() {
        let _forex_quote_mock = mock_response("/time_series?symbol=USD%2FRUB&interval=1min&outputsize=1&timezone=UTC&apikey=mock", indoc!(r#"
            {
                "meta": {
                    "currency_base": "US Dollar",
                    "currency_quote": "Russian Ruble",
                    "interval": "1min",
                    "symbol": "USD/RUB",
                    "type": "Physical Currency"
                },
                "status": "ok",
                "values": [
                    {
                        "close": "63.97370",
                        "datetime": "2020-01-31 21:58:00",
                        "high": "63.97500",
                        "low": "63.97310",
                        "open": "63.97310"
                    }
                ]
            }
        "#));

        let _stock_quote_mock = mock_response("/time_series?symbol=AMZN&interval=1min&outputsize=1&timezone=UTC&apikey=mock", indoc!(r#"
            {
                "meta": {
                    "currency": "USD",
                    "exchange": "NASDAQ",
                    "exchange_timezone": "America/New_York",
                    "interval": "1min",
                    "symbol": "AMZN",
                    "type": "Common Stock"
                },
                "status": "ok",
                "values": [
                    {
                        "close": "2007.76001",
                        "datetime": "2020-01-31 20:59:00",
                        "high": "2009.34497",
                        "low": "2007.76001",
                        "open": "2009.18005",
                        "volume": "96406"
                    }
                ]
            }
        "#));

        let _outdated_quote_mock = mock_response("/time_series?symbol=AAPL&interval=1min&outputsize=1&timezone=UTC&apikey=mock", indoc!(r#"
            {
                "meta": {
                    "currency": "USD",
                    "exchange": "NASDAQ",
                    "exchange_timezone": "America/New_York",
                    "interval": "1min",
                    "symbol": "AAPL",
                    "type": "Common Stock"
                },
                "status": "ok",
                "values": [
                    {
                        "close": "309.66000",
                        "datetime": "2020-01-31 20:58:00",
                        "high": "309.81000",
                        "low": "309.56000",
                        "open": "309.78000",
                        "volume": "314448"
                    }
                ]
            }
        "#));

        let _unknown_quote_mock = mock_response("/time_series?symbol=UNKNOWN&interval=1min&outputsize=1&timezone=UTC&apikey=mock", indoc!(r#"
            {
                "data": null,
                "message": "symbol_ticker not found",
                "status": "error"
            }
        "#));

        let client = TwelveData::new("mock");

        let mut quotes = HashMap::new();
        quotes.insert(s!("USD/RUB"), Cash::new("RUB", dec!(63.97370)));
        quotes.insert(s!("AMZN"), Cash::new("USD", dec!(2007.76001)));
        assert_eq!(client.get_quotes(&["USD/RUB", "UNKNOWN", "AMZN", "AAPL"]).unwrap(), quotes);
    }

    fn mock_response(path: &str, data: &str) -> Mock {
        mock("GET", path)
            .with_status(200)
            .with_header("Content-Type", "application/json")
            .with_body(data)
            .create()
    }
}