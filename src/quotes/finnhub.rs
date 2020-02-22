use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

#[cfg(test)] use indoc::indoc;
use log::trace;
#[cfg(test)] use mockito::{self, Mock, mock};
use num_traits::Zero;
use rayon::prelude::*;
use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::rate_limiter::RateLimiter;
use crate::util::{self, DecimalRestrictions};
use crate::types::Decimal;

use super::{QuotesMap, QuotesProvider};

pub struct Finnhub {
    token: String,
    client: Client,
    rate_limiter: RateLimiter,
}

impl Finnhub {
    pub fn new(token: &str) -> Finnhub {
        Finnhub {
            token: token.to_owned(),
            client: Client::new(),
            rate_limiter: RateLimiter::new()
                .with_limit(60 / 2, Duration::from_secs(60))
                .with_limit(30 / 2, Duration::from_secs(1)),
        }
    }

    fn get_quote(&self, symbol: &str) -> GenericResult<Option<Cash>> {
        #[derive(Deserialize)]
        struct Quote {
            #[serde(rename = "c")]
            current: Decimal,
        }

        let quote: Quote = self.query("quote", symbol)?;
        if quote.current.is_zero() {
            return Ok(None)
        }

        let price = util::validate_decimal(quote.current, DecimalRestrictions::StrictlyPositive)
            .map_err(|_| format!("Got an invalid {} price: {:?}", symbol, quote.current))?;

        // Profile API has too expensive rate limit weight, so try to avoid using it
        let currency = if symbol.contains('.') {
            #[derive(Deserialize)]
            struct Profile {
                currency: String,
            }

            self.query::<Profile>("stock/profile", symbol)?.currency
        } else {
            "USD".to_owned()
        };

        Ok(Some(Cash::new(&currency, price)))
    }

    fn query<T: DeserializeOwned>(&self, method: &str, symbol: &str) -> GenericResult<T> {
        #[cfg(not(test))] let base_url = "https://finnhub.io";
        #[cfg(test)] let base_url = mockito::server_url();

        let url = Url::parse_with_params(&format!("{}/api/v1/{}", base_url, method), &[
            ("symbol", symbol),
            ("token", self.token.as_ref()),
        ])?;

        let get = |url| -> GenericResult<T> {
            self.rate_limiter.wait(&format!("request to {}", url));

            trace!("Sending request to {}...", url);
            let response = self.client.get(url).send()?;
            trace!("Got response from {}.", url);

            if !response.status().is_success() {
                return Err!("Server returned an error: {}", response.status());
            }

            Ok(response.json()?)
        };

        Ok(get(url.as_str()).map_err(|e| format!(
            "Failed to get quotes from {}: {}", url, e))?)
    }
}

impl QuotesProvider for Finnhub {
    fn name(&self) -> &'static str {
        "Finnhub"
    }

    fn supports_forex(&self) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes() {
        let _bnd_profile_mock = mock_response("/api/v1/stock/profile?symbol=BND&token=mock", indoc!(r#"
            {
                "address": "100 Vanguard Boulevard, V26",
                "city": "Malvern",
                "country": "USA",
                "currency": "USD",
                "cusip": "921937835",
                "description": "Vanguard Bond Index Funds - Vanguard Total Bond Market ETF is an exchange traded fund launched and managed by The Vanguard Group, Inc. The fund invests in the fixed income markets of the United States.",
                "exchange": "NASDAQ-NMS Stock Market",
                "ggroup": "N/A",
                "gind": "N/A",
                "gsector": "N/A",
                "gsubind": "N/A",
                "ipo": "",
                "isin": "",
                "naics": "N/A",
                "name": "VANGUARD TOTAL BOND MARKET",
                "phone": "610-669-1000",
                "state": "PA",
                "ticker": "BND",
                "weburl": "advisors.vanguard.com"
            }
        "#));
        let _bnd_quote_mock = mock_response("/api/v1/quote?symbol=BND&token=mock", indoc!(r#"
            {
                "c": 84.91,
                "h": 85,
                "l": 84.8,
                "o": 84.83,
                "pc": 84.78
            }
        "#));

        let _unknown_profile_mock = mock_response("/api/v1/stock/profile?symbol=UNKNOWN&token=mock", indoc!(r#"
            {}
        "#));
        let _unknown_quote_mock = mock_response("/api/v1/quote?symbol=UNKNOWN&token=mock", indoc!(r#"
            {
                "c": 0,
                "h": 0,
                "l": 0,
                "o": 0,
                "pc": 0
            }
        "#));

        let _bndx_profile_mock = mock_response("/api/v1/stock/profile?symbol=BNDX&token=mock", indoc!(r#"
            {
                "address": "100 Vanguard Boulevard, V26",
                "city": "Malvern",
                "country": "USA",
                "currency": "USD",
                "cusip": "92203J407",
                "description": "Vanguard Charlotte Funds - Vanguard Total International Bond ETF is an exchange traded fund launched and managed by The Vanguard Group, Inc. The fund invests in the fixed income markets of countries across the globe excluding the United States. It primarily invests in non- U.S.",
                "exchange": "NASDAQ-NMS Stock Market",
                "ggroup": "N/A",
                "gind": "N/A",
                "gsector": "N/A",
                "gsubind": "N/A",
                "ipo": "",
                "isin": "",
                "naics": "Open-End Investment Funds",
                "name": "VANGUARD TOTAL INTERNATIONAL",
                "phone": "610-669-1000",
                "state": "PA",
                "ticker": "BNDX",
                "weburl": "advisors.vanguard.com"
            }
        "#));
        let _bndx_quote_mock = mock_response("/api/v1/quote?symbol=BNDX&token=mock", indoc!(r#"
            {
                "c": 57.26,
                "h": 57.29,
                "l": 57.2,
                "o": 57.21,
                "pc": 57.17
            }
        "#));

        let client = Finnhub::new("mock");

        let mut quotes = HashMap::new();
        quotes.insert(s!("BND"), Cash::new("USD", dec!(84.91)));
        quotes.insert(s!("BNDX"), Cash::new("USD", dec!(57.26)));
        assert_eq!(client.get_quotes(&["BND", "UNKNOWN", "BNDX"]).unwrap(), quotes);
    }

    fn mock_response(path: &str, data: &str) -> Mock {
        mock("GET", path)
            .with_status(200)
            .with_header("Content-Type", "application/json")
            .with_body(data)
            .create()
    }
}