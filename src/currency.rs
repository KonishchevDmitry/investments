use std::collections::HashSet;
use std::ops::Deref;
use std::ptr;
use std::str::FromStr;
use std::sync::Mutex;

#[cfg(test)] use mockito;
use reqwest::{self, Url};
use serde_xml_rs;

use core::GenericResult;
use types::{Date, Decimal};
use util;

#[cfg(not(test))]
const CBR_URL: &'static str = "http://www.cbr.ru";

#[cfg(test)]
const CBR_URL: &'static str = mockito::SERVER_URL;

lazy_static! {
    static ref CURRENCIES: Mutex<HashSet<&'static str>> = Mutex::new(HashSet::new());
}

#[derive(Debug)]
pub struct Cash {
    currency: &'static str,
    amount: Decimal,
}

impl Cash {
    pub fn new(currency: &str, amount: Decimal) -> Cash {
        Cash {
            currency: get_currency(currency),
            amount: amount,
        }
    }

    pub fn new_from_string(currency: &str, amount: &str) -> GenericResult<Cash> {
        Ok(Cash::new(currency, Decimal::from_str(amount).map_err(|_| format!(
            "Invalid cash amount: {:?}", amount))?))
    }
}

fn get_currency(currency: &str) -> &'static str {
    let mut currencies = CURRENCIES.lock().unwrap();

    match currencies.get(currency).map(|currency: &&str| *currency) {
        Some(static_currency) => static_currency,
        None => {
            let static_currency = Box::leak(currency.to_owned().into_boxed_str());
            currencies.insert(static_currency);
            static_currency
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CurrencyRate {
    date: Date,
    price: Decimal,
}

fn get_cbr_rates(currency: &str, start_date: Date, end_date: Date) -> GenericResult<Vec<CurrencyRate>> {
    let currency_code = "R01235"; // HACK: Don't hardcode
    if currency != "USD" {
        return Err!("{} currency is not supported yet.", currency);
    }

    let date_format = "%d/%m/%Y";
    let start_date_string = start_date.format(date_format).to_string();
    let end_date_string = end_date.format(date_format).to_string();

    let url = Url::parse_with_params(
        &(CBR_URL.to_owned() + "/scripts/XML_dynamic.asp"),
        &[
            ("date_req1", start_date_string.as_ref()),
            ("date_req2", end_date_string.as_ref()),
            ("VAL_NM_RQ", currency_code),
        ],
    )?;

    let get = |url| -> GenericResult<Vec<CurrencyRate>> {
        debug!("Getting {} currency rates for {} - {}...", currency, start_date, end_date);

        let mut response = reqwest::Client::new().get(url).send()?;
        if !response.status().is_success() {
            return Err!("The server returned an error: {}", response.status());
        }

        Ok(parse_cbr_rates(start_date, end_date, &response.text()?).map_err(|e| format!(
            "Rates info parsing error: {}", e))?)
    };

    Ok(get(url.as_str()).map_err(|e| format!(
        "Failed to get currency rates from {}: {}", url, e))?)
}

fn parse_cbr_rates(start_date: Date, end_date: Date, data: &str) -> GenericResult<Vec<CurrencyRate>> {
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
    let result: Rates = serde_xml_rs::deserialize(data.as_bytes())?;

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
            price: price / lot,
        })
    }

    Ok(rates)
}

#[cfg(test)]
mod tests {
    use mockito::{Mock, mock};

    use super::*;

    #[test]
    fn currency_cache() {
        let currencies = ["mock-1", "mock-2"];
        let mut cached_currencies = Vec::<&'static str>::new();

        for currency in currencies.iter().map(Deref::deref) {
            let cached_currency = get_currency(currency);
            cached_currencies.push(cached_currency);

            assert_eq!(cached_currency, currency);
            assert!(!ptr::eq(currency, cached_currency));
        }

        for (id, currency) in currencies.iter().enumerate() {
            assert!(ptr::eq(get_currency(currency), cached_currencies[id]));
        }
    }

    #[test]
    fn cbr_rates_empty() {
        let _mock = mock_cbr_response(
            "/scripts/XML_dynamic.asp?date_req1=02%2F09%2F2018&date_req2=03%2F09%2F2018&VAL_NM_RQ=R01235",
            indoc!(r#"
                <?xml version="1.0" encoding="windows-1251"?>
                <ValCurs ID="R01235" DateRange1="02.09.2018" DateRange2="03.09.2018" name="Foreign Currency Market Dynamic">
                </ValCurs>
            "#)
        );

        assert_eq!(
            get_cbr_rates("USD", Date::from_ymd(2018, 9, 2), Date::from_ymd(2018, 9, 3)).unwrap(),
            vec![],
        );
    }

    #[test]
    fn cbr_rates() {
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
            get_cbr_rates("USD", Date::from_ymd(2018, 9, 1), Date::from_ymd(2018, 9, 4)).unwrap(),
            vec![CurrencyRate {
                date: Date::from_ymd(2018, 9, 1),
                price: Decimal::from_str("68.0447").unwrap(),
            }, CurrencyRate {
                date: Date::from_ymd(2018, 9, 4),
                price: Decimal::from_str("67.7443").unwrap(),
            }],
        );
    }

    fn mock_cbr_response(path: &str, data: &str) -> Mock {
        return mock("GET", path)
            .with_status(200)
            .with_header("Content-Type", "application/xml; charset=windows-1251")
            .with_body(data)
            .create();
    }
}