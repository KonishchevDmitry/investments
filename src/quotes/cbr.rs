use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;

#[cfg(test)] use indoc::indoc;
use log::trace;
use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::core::GenericResult;
use crate::formats::xml;
use crate::quotes::CurrencyRate;
use crate::time;
use crate::types::{Date, Decimal};

pub const BASE_CURRENCY: &str = "RUB";

pub struct Cbr {
    url: String,
    client: Client,
    codes: Mutex<Option<HashMap<String, String>>>,
}

impl Cbr {
    pub fn new(url: &str) -> Cbr {
        Cbr {
            url: url.to_owned(),
            client: Client::new(),
            codes: Mutex::new(None),
        }
    }

    pub fn get_currency_rates(&self, currency: &str, start_date: Date, end_date: Date) -> GenericResult<Vec<CurrencyRate>> {
        #[derive(Deserialize)]
        struct Rate {
            #[serde(rename = "Date")]
            date: String,

            #[serde(rename = "Nominal")]
            lot: i32,

            #[serde(rename = "Value")]
            price: String,
        }

        #[derive(Deserialize)]
        struct Rates {
            #[serde(rename = "DateRange1")]
            start_date: String,

            #[serde(rename = "DateRange2")]
            end_date: String,

            #[serde(rename = "Record", default)]
            rates: Vec<Rate>
        }

        let request_date_format = "%d/%m/%Y";
        let start_date_string = start_date.format(request_date_format).to_string();
        let end_date_string = end_date.format(request_date_format).to_string();

        let result: Rates = self.query("currency rates", "XML_dynamic.asp", &[
            ("date_req1", start_date_string.as_str()),
            ("date_req2", end_date_string.as_str()),
            ("VAL_NM_RQ", &self.get_currency_code(currency)?),
        ])?;

        let response_date_format = "%d.%m.%Y";
        if time::parse_date(&result.start_date, response_date_format)? != start_date ||
            time::parse_date(&result.end_date, response_date_format)? != end_date {
            return Err!("The server returned currency rates info for an invalid period");
        }

        let mut rates = Vec::with_capacity(result.rates.len());

        for rate in result.rates {
            let lot = rate.lot;
            if lot <= 0 {
                return Err!("Invalid lot: {}", lot);
            }

            let price = rate.price.replace(',', ".");
            let price = Decimal::from_str(&price).map_err(|_| format!(
                "Invalid price: {:?}", rate.price))?;

            rates.push(CurrencyRate {
                date: time::parse_date(&rate.date, response_date_format)?,
                price: price / Decimal::from(lot),
            })
        }

        Ok(rates)
    }

    fn get_currency_code(&self, currency: &str) -> GenericResult<String> {
        #[derive(Deserialize)]
        struct Currency {
            #[serde(rename = "ID")]
            code: String,

            #[serde(rename = "ISO_Char_Code")]
            name: String,
        }

        #[derive(Deserialize)]
        struct Result {
            #[serde(rename = "Item")]
            currencies: Vec<Currency>,
        }

        let mut codes = self.codes.lock().unwrap();

        if codes.is_none() {
            let result: Result = self.query("currency codes", "XML_valFull.asp", &[("d", "0")])?;

            // Note: There may be several codes with the same name but different lot size
            codes.replace(result.currencies.into_iter().map(|Currency {code, name}| {
                (name, code)
            }).collect());
        }

        let code = codes.as_ref().unwrap().get(currency).ok_or_else(|| format!(
            "Invalid currency: {:?}", currency))?;

        Ok(code.clone())
    }

    fn query<T: DeserializeOwned>(&self, name: &str, method: &str, params: &[(&str, &str)]) -> GenericResult<T> {
        let url = Url::parse_with_params(&format!("{}/scripts/{}", self.url, method), params)?;
        let get = |url| -> GenericResult<T> {
            trace!("Sending request to {}...", url);
            let response = self.client.get(url).send()?;
            trace!("Got response from {}.", url);

            if !response.status().is_success() {
                return Err!("The server returned an error: {}", response.status());
            }

            xml::deserialize(response)
        };

        Ok(get(url.as_str()).map_err(|e| format!("Failed to get {} from {}: {}", name, url, e))?)
    }
}

#[cfg(test)]
mod tests {
    use mockito::{Server, ServerGuard, Mock};
    use super::*;

    #[test]
    fn empty_rates() {
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

        assert_eq!(client.get_currency_rates("USD", date!(2018, 9, 2), date!(2018, 9, 3)).unwrap(), vec![]);
    }

    #[test]
    fn rates() {
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
            client.get_currency_rates("USD", date!(2018, 9, 1), date!(2018, 9, 4)).unwrap(),
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
            client.get_currency_rates("JPY", date!(2018, 9, 1), date!(2018, 9, 4)).unwrap(),
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