use std::collections::HashMap;

#[cfg(test)] use indoc::indoc;
#[cfg(test)] use mockito::{self, Mock, mock};
use num_traits::FromPrimitive;
use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::{Deserializer, DeserializeOwned, Error};

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::util::{self, DecimalRestrictions};
use crate::types::Decimal;

use super::{QuotesMap, QuotesProvider};

pub struct Finnhub {
    token: String,
    client: Client,
}

impl Finnhub {
    pub fn new(token: &str) -> Finnhub {
        Finnhub {
            token: token.to_owned(),
            client: Client::new(),
        }
    }

    fn query<T: DeserializeOwned>(&self, method: &str, symbol: &str) -> GenericResult<T> {
        #[cfg(not(test))] let base_url = "https://finnhub.io";
        #[cfg(test)] let base_url = mockito::server_url();

        let url = Url::parse_with_params(&format!("{}/api/v1/{}", base_url, method), &[
            ("symbol", symbol),
            ("token", self.token.as_ref()),
        ])?;

        let get = |url| -> GenericResult<T> {
            let response = self.client.get(url).send()?;
            if !response.status().is_success() {
                return Err!("The server returned an error: {}", response.status());
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

    fn get_quotes(&self, symbols: &[String]) -> GenericResult<QuotesMap> {
        #[derive(Deserialize)]
        struct Profile {
            currency: Option<String>,
        }

        #[derive(Deserialize)]
        struct Quote {
            #[serde(rename = "c", deserialize_with = "deserialize_price")]
            current: Decimal,
        }

        let mut quotes = HashMap::new();

        for symbol in symbols {
            let profile: Profile = self.query("stock/profile", &symbol)?;
            let currency = match profile.currency {
                Some(currency) => currency,
                None => continue,
            };

            let quote: Quote = self.query("quote", &symbol)?;
            quotes.insert(symbol.clone(), Cash::new(&currency, quote.current));
        }

        Ok(quotes)
    }
}

fn deserialize_price<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
    where D: Deserializer<'de>
{
    let price: f64 = Deserialize::deserialize(deserializer)?;
    Ok(Decimal::from_f64(price).and_then(|price| {
        util::validate_decimal(price, DecimalRestrictions::StrictlyPositive).ok()
    }).ok_or_else(|| format!("Invalid price: {:?}", price)).map_err(D::Error::custom)?)
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
        assert_eq!(client.get_quotes(&[s!("BND"), s!("UNKNOWN"), s!("BNDX")]).unwrap(), quotes);
    }

    fn mock_response(path: &str, data: &str) -> Mock {
        mock("GET", path)
            .with_status(200)
            .with_header("Content-Type", "application/json")
            .with_body(data)
            .create()
    }
}