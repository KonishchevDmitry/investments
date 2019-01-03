use std::str::FromStr;

#[cfg(test)] use indoc::indoc;
use log::debug;
#[cfg(test)] use mockito::{self, Mock, mock};
use reqwest::{self, Url};
use serde_xml_rs;

use crate::core::GenericResult;
use crate::currency::CurrencyRate;
use crate::formatting;
use crate::types::{Date, Decimal};
use crate::util;

pub fn get_rates(currency: &str, start_date: Date, end_date: Date) -> GenericResult<Vec<CurrencyRate>> {
    let currency_code = match currency {
        "USD" => "R01235",
        _ => return Err!("{} currency is not supported yet.", currency),
    };

    let date_format = "%d/%m/%Y";
    let start_date_string = start_date.format(date_format).to_string();
    let end_date_string = end_date.format(date_format).to_string();

    #[cfg(not(test))]
    let base_url = "http://www.cbr.ru";

    #[cfg(test)]
    let base_url = mockito::server_url();

    let url = Url::parse_with_params(&format!("{}/scripts/XML_dynamic.asp", base_url), &[
        ("date_req1", start_date_string.as_ref()),
        ("date_req2", end_date_string.as_ref()),
        ("VAL_NM_RQ", currency_code),
    ])?;

    let get = |url| -> GenericResult<Vec<CurrencyRate>> {
        debug!("Getting {} currency rates for {} - {}...", currency,
               formatting::format_date(start_date), formatting::format_date(end_date));

        let mut response = reqwest::Client::new().get(url).send()?;
        if !response.status().is_success() {
            return Err!("The server returned an error: {}", response.status());
        }

        Ok(parse_rates(start_date, end_date, &response.text()?).map_err(|e| format!(
            "Rates info parsing error: {}", e))?)
    };

    Ok(get(url.as_str()).map_err(|e| format!(
        "Failed to get currency rates from {}: {}", url, e))?)
}

fn parse_rates(start_date: Date, end_date: Date, data: &str) -> GenericResult<Vec<CurrencyRate>> {
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

    let date_format = "%d.%m.%Y";
    let result: Rates = serde_xml_rs::from_str(data).map_err(|e| e.to_string())?;

    if util::parse_date(&result.start_date, date_format)? != start_date ||
        util::parse_date(&result.end_date, date_format)? != end_date {
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
            date: util::parse_date(&rate.date, date_format)?,
            price: price / Decimal::from(lot),
        })
    }

    Ok(rates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rates() {
        let _mock = mock_cbr_response(
            "/scripts/XML_dynamic.asp?date_req1=02%2F09%2F2018&date_req2=03%2F09%2F2018&VAL_NM_RQ=R01235",
            indoc!(r#"
                <?xml version="1.0" encoding="windows-1251"?>
                <ValCurs ID="R01235" DateRange1="02.09.2018" DateRange2="03.09.2018" name="Foreign Currency Market Dynamic">
                </ValCurs>
            "#)
        );

        assert_eq!(get_rates("USD", date!(2, 9, 2018), date!(3, 9, 2018)).unwrap(), vec![]);
    }

    #[test]
    fn rates() {
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
            get_rates("USD", date!(1, 9, 2018), date!(4, 9, 2018)).unwrap(),
            vec![CurrencyRate {
                date: date!(1, 9, 2018),
                price: decf!(68.0447),
            }, CurrencyRate {
                date: date!(4, 9, 2018),
                price: decf!(67.7443),
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