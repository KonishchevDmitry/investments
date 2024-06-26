use std::collections::HashMap;
use std::str::FromStr;
use std::sync::OnceLock;

#[cfg(test)] use indoc::indoc;
use log::warn;
use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::{DeserializeOwned, Deserializer, Error};
use validator::{Validate, ValidationError};

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::formats::xml;
use crate::formatting;
use crate::forex;
use crate::localities;
use crate::quotes::{CurrencyRate, QuotesMap, QuotesProvider};
use crate::time;
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

use super::common::send_request;

pub const BASE_CURRENCY: &str = "RUB";

pub struct Cbr {
    url: String,
    client: Client,
    codes: OnceLock<GenericResult<HashMap<String, String>>>,
    rates: OnceLock<GenericResult<HashMap<String, Decimal>>>,
}

impl Cbr {
    pub fn new(url: &str) -> Cbr {
        Cbr {
            url: url.to_owned(),
            client: Client::new(),
            codes: OnceLock::new(),
            rates: OnceLock::new(),
        }
    }

    fn get_currency_rates(&self) -> GenericResult<HashMap<String, Decimal>> {
        #[derive(Deserialize, Validate)]
        struct Rates {
            #[serde(rename = "Date", deserialize_with = "deserialize_date")]
            date: Date,

            #[validate(nested)]
            #[serde(rename = "Valute")]
            rates: Vec<Rate>
        }

        #[derive(Deserialize, Validate)]
        struct Rate {
            #[serde(rename = "CharCode")]
            symbol: String,

            #[validate(custom(function = "validate_price"))]
            #[serde(rename = "VunitRate", deserialize_with = "deserialize_price")]
            price: Decimal,
        }

        let result: Rates = self.query("currency rates", "XML_daily.asp", &[])?;
        if result.date < localities::get_russian_central_bank_min_last_working_day(time::today()) {
            warn!("Got outdated ({}) currency rates from CBR.", formatting::format_date(result.date));
        }

        Ok(result.rates.into_iter().map(|rate| (rate.symbol, rate.price)).collect())
    }

    pub fn get_historical_currency_rates(&self, currency: &str, start_date: Date, end_date: Date) -> GenericResult<Vec<CurrencyRate>> {
        #[derive(Deserialize, Validate)]
        struct Rates {
            #[serde(rename = "DateRange1", deserialize_with = "deserialize_date")]
            start_date: Date,

            #[serde(rename = "DateRange2", deserialize_with = "deserialize_date")]
            end_date: Date,

            #[validate(nested)]
            #[serde(rename = "Record", default)]
            rates: Vec<Rate>
        }

        #[derive(Deserialize, Validate)]
        struct Rate {
            #[serde(rename = "Date", deserialize_with = "deserialize_date")]
            date: Date,

            #[validate(range(min = 1))]
            #[serde(rename = "Nominal")]
            lot: i32,

            #[validate(custom(function = "validate_price"))]
            #[serde(rename = "Value", deserialize_with = "deserialize_price")]
            price: Decimal,
        }

        let request_date_format = "%d/%m/%Y";
        let start_date_string = start_date.format(request_date_format).to_string();
        let end_date_string = end_date.format(request_date_format).to_string();

        let result: Rates = self.query("currency rates", "XML_dynamic.asp", &[
            ("date_req1", start_date_string.as_str()),
            ("date_req2", end_date_string.as_str()),
            ("VAL_NM_RQ", &self.get_currency_code(currency)?),
        ])?;

        if result.start_date != start_date || result.end_date != end_date {
            return Err!("The server returned currency rates info for an invalid period");
        }

        Ok(result.rates.into_iter().map(|rate| {
            CurrencyRate {
                date: rate.date,
                price: rate.price / Decimal::from(rate.lot),
            }
        }).collect())
    }

    fn get_currency_code(&self, currency: &str) -> GenericResult<String> {
        #[derive(Deserialize, Validate)]
        struct Result {
            #[serde(rename = "Item")]
            currencies: Vec<Currency>,
        }

        #[derive(Deserialize)]
        struct Currency {
            #[serde(rename = "ID")]
            code: String,

            #[serde(rename = "ISO_Char_Code")]
            name: String,
        }

        let codes = self.codes.get_or_init(|| {
            let result: Result = self.query("currency codes", "XML_valFull.asp", &[("d", "0")])?;

            // Note: There may be several codes with the same name but different lot size
            Ok(result.currencies.into_iter().map(|Currency {code, name}| {
                (name, code)
            }).collect())
        }).as_ref().map_err(|e| e.to_string())?;

        let code = codes.get(currency).ok_or_else(|| format!(
            "Invalid currency: {:?}", currency))?;

        Ok(code.clone())
    }

    fn query<T: DeserializeOwned + Validate>(&self, name: &str, method: &str, params: &[(&str, &str)]) -> GenericResult<T> {
        let url = format!("{}/scripts/{}", self.url, method);

        let url = if params.is_empty() {
            Url::parse(&url)? // parse_with_params() adds trailing '?' to the end of the URL
        } else {
            Url::parse_with_params(&url, params)?
        };

        let get = |url| -> GenericResult<T> {
            let response = send_request(&self.client, url,  None)?;

            let result: T = xml::deserialize(response)?;
            result.validate()?;

            Ok(result)
        };

        Ok(get(url.as_str()).map_err(|e| format!("Failed to get {} from {}: {}", name, url, e))?)
    }
}

impl QuotesProvider for Cbr {
    fn name(&self) -> &'static str {
        "CBR"
    }

    fn supports_forex(&self) -> bool {
        true
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        let rates = self.rates.get_or_init(|| {
            self.get_currency_rates()
        }).as_ref().map_err(|e| e.to_string())?;

        let mut quotes = QuotesMap::new();

        for &symbol in symbols {
            let (base, quote) = forex::parse_currency_pair(symbol)?;
            if let Some(quote) = get_quote(base, quote, rates) {
                quotes.insert(symbol.to_owned(), quote);
            }
        }

        Ok(quotes)
    }
}

fn deserialize_date<'de, D>(deserializer: D) -> Result<Date, D::Error>
    where D: Deserializer<'de>
{
    let date: String = Deserialize::deserialize(deserializer)?;
    time::parse_date(&date, "%d.%m.%Y").map_err(D::Error::custom)
}

fn deserialize_price<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
    where D: Deserializer<'de>
{
    let price: String = Deserialize::deserialize(deserializer)?;
    Decimal::from_str(&price.replace(',', ".")).ok()
        .and_then(|price| if price > dec!(0) {
            Some(price)
        } else {
            None
        })
        .ok_or_else(|| D::Error::custom(format!("Invalid price: {:?}", price)))
}

fn validate_price(&price: &Decimal) -> Result<(), ValidationError> {
    if util::validate_decimal(price, DecimalRestrictions::StrictlyPositive).is_err() {
        return Err(ValidationError::new("price").with_message(format!("Invalid price: {}", price).into()));
    }
    Ok(())
}

fn get_quote(base: &str, quote: &str, rates: &HashMap<String, Decimal>) -> Option<Cash> {
    let base_rate = match base {
        BASE_CURRENCY => dec!(1),
        _ => *rates.get(base)?,
    };

    let quote_rate = match quote {
        BASE_CURRENCY => dec!(1),
        _ => *rates.get(quote)?,
    };

    Some(Cash::new(quote, base_rate / quote_rate))
}

#[cfg(test)]
mod tests {
    use mockito::{Server, ServerGuard, Mock};

    use super::*;

    #[test]
    fn rates() {
        let (mut server, client) = create_server();

        let _rates_mock = mock_response(
            &mut server, "/scripts/XML_daily.asp",
            indoc!(r#"
                <?xml version="1.0" encoding="windows-1251"?>
                <ValCurs Date="27.06.2024" name="Foreign Currency Market">
                    <Valute ID="R01235">
                        <NumCode>840</NumCode>
                        <CharCode>USD</CharCode>
                        <Nominal>1</Nominal>
                        <Name>Доллар США</Name>
                        <Value>87,8064</Value>
                        <VunitRate>87,8064</VunitRate>
                    </Valute>
                    <Valute ID="R01375">
                        <NumCode>156</NumCode>
                        <CharCode>CNY</CharCode>
                        <Nominal>1</Nominal>
                        <Name>Китайский юань</Name>
                        <Value>11,8748</Value>
                        <VunitRate>11,8748</VunitRate>
                    </Valute>
                    <Valute ID="R01150">
                        <NumCode>704</NumCode>
                        <CharCode>VND</CharCode>
                        <Nominal>10000</Nominal>
                        <Name>Вьетнамских донгов</Name>
                        <Value>36,1969</Value>
                        <VunitRate>0,00361969</VunitRate>
                    </Valute>
                </ValCurs>
            "#)
        );

        let mut quotes = client.get_quotes(&["RUB/RUB", "USD/RUB", "RUB/USD", "VND/RUB", "RUB/VND", "USD/VND", "VND/USD", "XXX/YYY"]).unwrap();
        quotes = quotes.into_iter().map(|(symbol, quote)| (symbol, quote.round_to(6))).collect();

        assert_eq!(
            quotes,
            hashmap!{
                s!("RUB/RUB") => Cash::new("RUB", dec!(1)),

                s!("USD/RUB") => Cash::new("RUB", dec!(87.8064)),
                s!("RUB/USD") => Cash::new("USD", dec!(0.011389)),

                s!("VND/RUB") => Cash::new("RUB", dec!(0.00362)),
                s!("RUB/VND") => Cash::new("VND", dec!(276.266752)),

                s!("USD/VND") => Cash::new("VND", dec!(24257.988944)),
                s!("VND/USD") => Cash::new("USD", dec!(0.000041)),
            },
        );
    }

    #[test]
    fn empty_historical_rates() {
        let (mut server, client) = create_server();

        let _currencies_mock = mock_currencies(&mut server);
        let _usd_mock = mock_response(
            &mut server, "/scripts/XML_dynamic.asp?date_req1=02%2F09%2F2018&date_req2=03%2F09%2F2018&VAL_NM_RQ=R01235",
            indoc!(r#"
                <?xml version="1.0" encoding="windows-1251"?>
                <ValCurs ID="R01235" DateRange1="02.09.2018" DateRange2="03.09.2018" name="Foreign Currency Market Dynamic">
                </ValCurs>
            "#)
        );

        assert_eq!(client.get_historical_currency_rates("USD", date!(2018, 9, 2), date!(2018, 9, 3)).unwrap(), vec![]);
    }

    #[test]
    fn historical_rates() {
        let (mut server, client) = create_server();

        let _currencies_mock = mock_currencies(&mut server);
        let _usd_mock = mock_response(
            &mut server, "/scripts/XML_dynamic.asp?date_req1=01%2F09%2F2018&date_req2=04%2F09%2F2018&VAL_NM_RQ=R01235",
            indoc!(r#"
                <?xml version="1.0" encoding="windows-1251"?>
                <ValCurs ID="R01235" DateRange1="01.09.2018" DateRange2="04.09.2018" name="Foreign Currency Market Dynamic">
                    <Record Date="01.09.2018" Id="R01235">
                        <Nominal>1</Nominal>
                        <Value>68,0447</Value>
                    </Record>
                    <Record Date="04.09.2018" Id="R01235">
                        <Nominal>1</Nominal>
                        <Value>67,7443</Value>
                    </Record>
                </ValCurs>
            "#)
        );

        assert_eq!(
            client.get_historical_currency_rates("USD", date!(2018, 9, 1), date!(2018, 9, 4)).unwrap(),
            vec![CurrencyRate {
                date: date!(2018, 9, 1),
                price: dec!(68.0447),
            }, CurrencyRate {
                date: date!(2018, 9, 4),
                price: dec!(67.7443),
            }],
        );

        let _jpy_mock = mock_response(
            &mut server, "/scripts/XML_dynamic.asp?date_req1=01%2F09%2F2018&date_req2=04%2F09%2F2018&VAL_NM_RQ=R01820",
            indoc!(r#"
                <?xml version="1.0" encoding="windows-1251"?>
                <ValCurs ID="R01820" DateRange1="01.09.2018" DateRange2="04.09.2018" name="Foreign Currency Market Dynamic">
                    <Record Date="01.09.2018" Id="R01820">
                        <Nominal>100</Nominal>
                        <Value>61,4704</Value>
                    </Record>
                    <Record Date="04.09.2018" Id="R01820">
                        <Nominal>100</Nominal>
                        <Value>61,0172</Value>
                    </Record>
                </ValCurs>
            "#)
        );

        assert_eq!(
            client.get_historical_currency_rates("JPY", date!(2018, 9, 1), date!(2018, 9, 4)).unwrap(),
            vec![CurrencyRate {
                date: date!(2018, 9, 1),
                price: dec!(0.614704),
            }, CurrencyRate {
                date: date!(2018, 9, 4),
                price: dec!(0.610172),
            }],
        );
    }

    fn create_server() -> (ServerGuard, Cbr) {
        let server = Server::new();
        let client = Cbr::new(&server.url());
        (server, client)
    }

    fn mock_currencies(server: &mut Server) -> Mock {
        mock_response(
            server, "/scripts/XML_valFull.asp?d=0",
            indoc!(r#"
                <?xml version="1.0" encoding="windows-1251"?>
                <Valuta name="Foreign Currency Market Lib">
                    <Item ID="R01235">
                        <Name>Доллар США</Name>
                        <EngName>US Dollar</EngName>
                        <Nominal>1</Nominal>
                        <ParentCode>R01235 </ParentCode>
                        <ISO_Num_Code>840</ISO_Num_Code>
                        <ISO_Char_Code>USD</ISO_Char_Code>
                    </Item>
                    <Item ID="R01239">
                        <Name>Евро</Name>
                        <EngName>Euro</EngName>
                        <Nominal>1</Nominal>
                        <ParentCode>R01239 </ParentCode>
                        <ISO_Num_Code>978</ISO_Num_Code>
                        <ISO_Char_Code>EUR</ISO_Char_Code>
                    </Item>
                    <Item ID="R01510">
                        <Name>Немецкая марка</Name>
                        <EngName>Deutsche Mark</EngName>
                        <Nominal>1</Nominal>
                        <ParentCode>R01510 </ParentCode>
                        <ISO_Num_Code>276</ISO_Num_Code>
                        <ISO_Char_Code>DEM</ISO_Char_Code>
                    </Item>
                    <Item ID="R01510A">
                        <Name>Немецкая марка</Name>
                        <EngName>Deutsche Mark</EngName>
                        <Nominal>100</Nominal>
                        <ParentCode>R01510 </ParentCode>
                        <ISO_Num_Code>280</ISO_Num_Code>
                        <ISO_Char_Code>DEM</ISO_Char_Code>
                    </Item>
                    <Item ID="R01820">
                        <Name>Японская иена</Name>
                        <EngName>Japanese Yen</EngName>
                        <Nominal>100</Nominal>
                        <ParentCode>R01820 </ParentCode>
                        <ISO_Num_Code>392</ISO_Num_Code>
                        <ISO_Char_Code>JPY</ISO_Char_Code>
                    </Item>
                </Valuta>
            "#)
        )
    }

    fn mock_response(server: &mut Server, path: &str, data: &str) -> Mock {
        let (data, _, errors) = encoding_rs::WINDOWS_1251.encode(data);
        assert!(!errors);

        server.mock("GET", path)
            .with_status(200)
            .with_header("Content-Type", "application/xml; charset=windows-1251")
            .with_body(data)
            .create()
    }
}