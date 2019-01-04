use std::collections::HashMap;
#[cfg(test)] use std::fs::File;
#[cfg(test)] use std::path::Path;

use log::error;
#[cfg(test)] use mockito::{self, Mock, mock};
use num_traits::Zero;
use reqwest::{Client, Url};

use crate::core::GenericResult;
use crate::currency::Cash;
#[cfg(not(test))]use crate::localities;
use crate::types::{Decimal, Date};
use crate::util;

use super::{QuotesMap, QuotesProvider};

pub struct Moex {
}

impl Moex {
    pub fn new() -> Moex {
        Moex {}
    }
}

impl QuotesProvider for Moex {
    fn name(&self) -> &'static str {
        "Moscow Exchange"
    }

    fn get_quotes(&self, symbols: &Vec<String>) -> GenericResult<QuotesMap> {
        #[cfg(not(test))]
        let base_url = "https://iss.moex.com";

        #[cfg(test)]
        let base_url = mockito::server_url();

        let url = Url::parse_with_params(
            &format!("{}/iss/engines/stock/markets/shares/boards/TQTF/securities.xml", base_url),
            &[("securities", symbols.join(",").as_str())],
        )?;

        let get = |url| -> GenericResult<HashMap<String, Cash>> {
            let mut response = Client::new().get(url).send()?;
            if !response.status().is_success() {
                return Err!("The server returned an error: {}", response.status());
            }

            Ok(parse_quotes(&response.text()?).map_err(|e| format!(
                "Quotes info parsing error: {}", e))?)
        };

        Ok(get(url.as_str()).map_err(|e| format!(
            "Failed to get quotes from {}: {}", url, e))?)
    }
}

fn parse_quotes(data: &str) -> GenericResult<HashMap<String, Cash>> {
    #[derive(Deserialize)]
    struct Document {
        data: Vec<Data>,
    }

    #[derive(Deserialize)]
    struct Data {
        id: String,

        #[serde(rename = "rows")]
        table: Table,
    }

    #[derive(Deserialize)]
    struct Table {
        #[serde(rename = "row", default)]
        rows: Vec<Row>,
    }

    #[derive(Deserialize)]
    struct Row {
        #[serde(rename = "SECID")]
        symbol: Option<String>,

        #[serde(rename = "LOTSIZE")]
        lot_size: Option<u32>,

        #[serde(rename = "LAST")]
        price: Option<Decimal>,

        #[serde(rename = "CURRENCYID")]
        currency: Option<String>,

        // Time columns behaviour:
        // * 10.11.2018 closed session: UPDATETIME="19:18:26" TIME="18:41:07" SYSTIME="2018-11-09 19:33:27"
        // * 13.11.2018 open session: UPDATETIME="13:00:50" TIME="13:00:30" SYSTIME="2018-11-13 13:15:50"
        #[serde(rename = "SYSTIME")]
        time: Option<String>,
    }

    let result: Document = serde_xml_rs::from_str(data).map_err(|e| e.to_string())?;
    let (mut securities, mut market_data) = (None, None);

    for data in &result.data {
        let data_ref = match data.id.as_str() {
            "securities" => &mut securities,
            "marketdata" => &mut market_data,
            _ => continue,
        };

        if data_ref.is_some() {
            return Err!("Duplicated {:?} data", data.id);
        }

        *data_ref = Some(&data.table.rows);
    }

    let (securities, market_data) = match (securities, market_data) {
        (Some(securities), Some(market_data)) => (securities, market_data),
        _ => return Err!("Unable to find securities info in server response"),
    };

    let mut symbols = HashMap::new();

    for row in securities {
        let symbol = get_value(&row.symbol)?;
        let lot_size = get_value(&row.lot_size)?;
        let currency = get_value(&row.currency)?;

        if *lot_size != 1 {
            return Err!("{} has lot = {} which is not supported yet", symbol, lot_size);
        }

        let currency = match currency.as_str() {
            "SUR" => "RUB",
            _ => return Err!("{} is nominated in an unsupported currency: {}", symbol, currency),
        };

        if symbols.insert(symbol, currency).is_some() {
            return Err!("Duplicated symbol: {}", symbol);
        }
    }

    let mut quotes = HashMap::new();
    let mut outdated = Vec::new();

    for row in market_data {
        let symbol = get_value(&row.symbol)?;

        let date = util::parse_date_time(&get_value(&row.time)?, "%Y-%m-%d %H:%M:%S")?.date();
        if is_outdated(date) {
            outdated.push(symbol.clone());
            continue;
        }

        let currency = symbols.get(symbol).ok_or_else(|| format!(
            "There is market data for {} but security info is missing", symbol))?;

        let price = get_value(&row.price)?;
        if price.is_zero() || price.is_sign_negative() {
            return Err!("Invalid price: {}", price);
        }

        if quotes.insert(symbol.clone(), Cash::new(currency, *price)).is_some() {
            return Err!("Duplicated symbol: {}", symbol);
        }
    }

    if !outdated.is_empty() {
        error!("Got outdated quotes for the following symbols: {}.", outdated.join(", "));
    }

    Ok(quotes)
}

fn get_value<T>(value: &Option<T>) -> GenericResult<&T> {
    Ok(value.as_ref().ok_or_else(|| "Got an unexpected response from server")?)
}

#[cfg(not(test))]
fn is_outdated(date: Date) -> bool {
    date < localities::get_russian_stock_exchange_min_last_working_day(util::today())
}

#[cfg(test)]
fn is_outdated(_date: Date) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use super::*;

    #[test]
    fn no_quotes() {
        let _mock = mock_response(
            "/iss/engines/stock/markets/shares/boards/TQTF/securities.xml?securities=FXUS%2CFXIT",
            "testdata/moex-empty.xml",
        );

        assert_eq!(Moex::new().get_quotes(&vec![s!("FXUS"), s!("FXIT")]).unwrap(), HashMap::new());
    }

    #[test]
    fn quotes() {
        let _mock = mock_response(
            "/iss/engines/stock/markets/shares/boards/TQTF/securities.xml?securities=FXUS%2CFXIT%2CINVALID",
            "testdata/moex.xml",
        );

        let mut quotes = HashMap::new();
        quotes.insert(s!("FXUS"), Cash::new("RUB", dec!(3320)));
        quotes.insert(s!("FXIT"), Cash::new("RUB", dec!(4612)));

        assert_eq!(Moex::new().get_quotes(&vec![
            s!("FXUS"), s!("FXIT"), s!("INVALID")
        ]).unwrap(), quotes);
    }

    fn mock_response(path: &str, body_path: &str) -> Mock {
        let body_path = Path::new(file!()).parent().unwrap().join(body_path);

        let mut body = String::new();
        File::open(body_path).unwrap().read_to_string(&mut body).unwrap();

        mock("GET", path)
            .with_status(200)
            .with_header("Content-Type", "application/xml; charset=utf-8")
            .with_body(body)
            .create()
    }
}