use std::str::FromStr;

#[cfg(test)] use indoc::indoc;
use log::trace;
#[cfg(test)] use mockito::{self, Mock, mock};
use reqwest::Url;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::core::GenericResult;
use crate::currency::CurrencyRate;
use crate::types::{Date, Decimal};
use crate::util;

pub const BASE_CURRENCY: &str = "RUB";

pub struct Cbr {
    client: Client,
}

impl Cbr {
    pub fn new() -> Cbr {
        Cbr {
            client: Client::new(),
        }
    }

    pub fn get_rates(&self, currency: &str, start_date: Date, end_date: Date) -> GenericResult<Vec<CurrencyRate>> {
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
            ("VAL_NM_RQ", self.get_currency_code(currency)?),
        ])?;

        let response_date_format = "%d.%m.%Y";
        if util::parse_date(&result.start_date, response_date_format)? != start_date ||
            util::parse_date(&result.end_date, response_date_format)? != end_date {
            return Err!("The server returned currency rates info for an invalid period");
        }

        let mut rates = Vec::with_capacity(result.rates.len());

        for rate in result.rates {
            let lot = rate.lot;
            if lot <= 0 {
                return Err!("Invalid lot: {}", lot);
            }

            let price = rate.price.replace(",", ".");
            let price = Decimal::from_str(&price).map_err(|_| format!(
                "Invalid price: {:?}", rate.price))?;

            rates.push(CurrencyRate {
                date: util::parse_date(&rate.date, response_date_format)?,
                price: price / Decimal::from(lot),
            })
        }

        Ok(rates)
    }

    fn get_currency_code(&self, currency: &str) -> GenericResult<&str> {
        Ok(match currency {
            "USD" => "R01235",
            "EUR" => "R01239",
            _ => return Err!("{} currency is not supported yet", currency),
        })
    }

    fn query<T: DeserializeOwned>(&self, name: &str, method: &str, params: &[(&str, &str)]) -> GenericResult<T> {
        #[cfg(not(test))] let base_url = "http://www.cbr.ru";
        #[cfg(test)] let base_url = mockito::server_url();

        let url = Url::parse_with_params(&format!("{}/scripts/{}", base_url, method), params)?;
        let get = |url| -> GenericResult<T> {
            trace!("Sending request to {}...", url);
            let response = self.client.get(url).send()?;
            trace!("Got response from {}.", url);

            if !response.status().is_success() {
                return Err!("The server returned an error: {}", response.status());
            }

            Ok(serde_xml_rs::from_str(&response.text()?)?)
        };

        Ok(get(url.as_str()).map_err(|e| format!("Failed to get {} from {}: {}", name, url, e))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rates() {
        let cbr = Cbr::new();
        let _mock = mock_cbr_response(
            "/scripts/XML_dynamic.asp?date_req1=02%2F09%2F2018&date_req2=03%2F09%2F2018&VAL_NM_RQ=R01235",
            indoc!(r#"
                <?xml version="1.0" encoding="windows-1251"?>
                <ValCurs ID="R01235" DateRange1="02.09.2018" DateRange2="03.09.2018" name="Foreign Currency Market Dynamic">
                </ValCurs>
            "#)
        );

        assert_eq!(cbr.get_rates("USD", date!(2, 9, 2018), date!(3, 9, 2018)).unwrap(), vec![]);
    }

    #[test]
    fn rates() {
        let cbr = Cbr::new();
        let _mock = mock_cbr_response(
            "/scripts/XML_dynamic.asp?date_req1=01%2F09%2F2018&date_req2=04%2F09%2F2018&VAL_NM_RQ=R01235",
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
            cbr.get_rates("USD", date!(1, 9, 2018), date!(4, 9, 2018)).unwrap(),
            vec![CurrencyRate {
                date: date!(1, 9, 2018),
                price: dec!(68.0447),
            }, CurrencyRate {
                date: date!(4, 9, 2018),
                price: dec!(67.7443),
            }],
        );
    }

    fn mock_cbr_response(path: &str, data: &str) -> Mock {
        mock("GET", path)
            .with_status(200)
            .with_header("Content-Type", "application/xml; charset=windows-1251")
            .with_body(data)
            .create()
    }
}