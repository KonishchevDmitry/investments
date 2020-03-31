use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
#[cfg(test)] use indoc::indoc;
use log::{debug, trace};
#[cfg(test)] use mockito::{self, Mock, mock};
use num_traits::Zero;
use rayon::prelude::*;
use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json;

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
            #[serde(rename = "t")]
            day_start_time: Option<i64>,

            #[serde(rename = "c")]
            current_price: Decimal,
        }

        let quote = match self.query::<Quote>("quote", symbol)? {
            Some(quote) if !quote.current_price.is_zero() => quote,
            _ => return Ok(None),
        };

        if let Some(time) = quote.day_start_time {
            if is_outdated(time)? {
                let time = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(time, 0), Utc);
                debug!("{}: Got outdated quotes: {}.", symbol, time);
                return Ok(None);
            }
        } else {
            return Ok(None);
        }

        let price = util::validate_decimal(quote.current_price, DecimalRestrictions::StrictlyPositive)
            .map_err(|_| format!("Got an invalid {} price: {:?}", symbol, quote.current_price))?;

        // Profile API has too expensive rate limit weight, so try to avoid using it
        let currency = if symbol.contains('.') {
            #[derive(Deserialize)]
            struct Profile {
                currency: String,
            }

            let profile = match self.query::<Profile>("stock/profile", symbol)? {
                Some(profile) => profile,
                None => return Ok(None),
            };

            profile.currency
        } else {
            "USD".to_owned()
        };

        Ok(Some(Cash::new(&currency, price)))
    }

    fn query<T: DeserializeOwned>(&self, method: &str, symbol: &str) -> GenericResult<Option<T>> {
        #[cfg(not(test))] let base_url = "https://finnhub.io";
        #[cfg(test)] let base_url = mockito::server_url();

        let url = Url::parse_with_params(&format!("{}/api/v1/{}", base_url, method), &[
            ("symbol", symbol),
            ("token", self.token.as_ref()),
        ])?;

        let get = |url| -> GenericResult<Option<T>> {
            self.rate_limiter.wait(&format!("request to {}", url));

            trace!("Sending request to {}...", url);
            let response = self.client.get(url).send()?;
            trace!("Got response from {}.", url);

            if !response.status().is_success() {
                return Err!("Server returned an error: {}", response.status());
            }
            let reply = response.text()?;

            if reply.trim() == "Symbol not supported" {
                return Ok(None);
            }

            Ok(serde_json::from_str(&reply)?)
        };

        Ok(get(url.as_str()).map_err(|e| format!(
            "Failed to get quotes from {}: {}", url, e))?)
    }
}

impl QuotesProvider for Finnhub {
    fn name(&self) -> &'static str {
        "Finnhub"
    }

    fn high_precision(&self) -> bool {
        true
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

#[cfg(not(test))]
fn is_outdated(time: i64) -> GenericResult<bool> {
    let date_time = NaiveDateTime::from_timestamp_opt(time, 0).ok_or_else(|| format!(
        "Got an invalid UNIX time: {}", time))?;
    Ok(super::is_outdated_quote::<Utc>(DateTime::from_utc(date_time, Utc)))
}

#[cfg(test)]
fn is_outdated(time: i64) -> GenericResult<bool> {
    #![allow(clippy::unreadable_literal)]
    Ok(time < 1582295400)
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
                "c": 85.80000305175781,
                "h": 85.93000030517578,
                "l": 85.7300033569336,
                "o": 85.76000213623047,
                "pc": 85.58999633789062,
                "t": 1582295400
            }
        "#));

        let _outdated_profile_mock = mock_response("/api/v1/stock/profile?symbol=AMZN&token=mock", indoc!(r#"
            {
                "address": "410 Terry Avenue North",
                "city": "Seattle",
                "country": "USA",
                "currency": "USD",
                "cusip": "023135106",
                "description": "Amazon.com, Inc. engages in the retail sale of consumer products and subscriptions in North America and internationally. The company operates through three segments: North America, International, and Amazon Web Services (AWS) segments.",
                "exchange": "NASDAQ-NMS Stock Market",
                "ggroup": "Retailing",
                "gind": "Internet & Direct Marketing Retail",
                "gsector": "Consumer Discretionary",
                "gsubind": "Internet & Direct Marketing Retail",
                "ipo": "1997-05-15",
                "isin": "",
                "naics": "",
                "name": "AMAZON.COM INC",
                "phone": "206-266-1000",
                "state": "WA",
                "ticker": "AMZN",
                "weburl": "www.amazon.com"
            }
        "#));
        let _outdated_quote_mock = mock_response("/api/v1/quote?symbol=AMZN&token=mock", indoc!(r#"
            {
                "c": 2095.969970703125,
                "h": 2144.550048828125,
                "l": 2088,
                "o": 2142.14990234375,
                "pc": 2153.10009765625,
                "t": 1
            }
        "#));

        // Old response for unknown symbols
        let _unknown_old_profile_mock = mock_response("/api/v1/stock/profile?symbol=UNKNOWN_OLD&token=mock", "{}");
        let _unknown_old_quote_mock = mock_response("/api/v1/quote?symbol=UNKNOWN_OLD&token=mock", indoc!(r#"
            {
                "c": 0,
                "h": 0,
                "l": 0,
                "o": 0,
                "pc": 0
            }
        "#));

        let _unknown_profile_mock = mock_response("/api/v1/stock/profile?symbol=UNKNOWN&token=mock", "{}");
        let _unknown_quote_mock = mock_response("/api/v1/quote?symbol=UNKNOWN&token=mock", "Symbol not supported");

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
                "c": 57.86000061035156,
                "h": 57.900001525878906,
                "l": 57.849998474121094,
                "o": 57.86000061035156,
                "pc": 57.7599983215332,
                "t": 1582295400
            }
        "#));

        let client = Finnhub::new("mock");

        let mut quotes = HashMap::new();
        quotes.insert(s!("BND"), Cash::new("USD", dec!(85.80000305175781)));
        quotes.insert(s!("BNDX"), Cash::new("USD", dec!(57.86000061035156)));
        assert_eq!(client.get_quotes(&["BND", "AMZN", "UNKNOWN_OLD", "UNKNOWN", "BNDX"]).unwrap(), quotes);
    }

    fn mock_response(path: &str, data: &str) -> Mock {
        // All responses are always 200 OK, some of them are returned with application/json content
        // type, some - with text/plain even for JSON payload.
        mock("GET", path)
            .with_status(200)
            .with_body(data)
            .create()
    }
}