use std::collections::{BTreeMap, HashMap, HashSet};
use std::str::FromStr;
use std::time::Duration;

use itertools::Itertools;
use log::{error, debug};
use num_traits::FromPrimitive;
use reqwest::Url;
use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use serde::de::{Deserializer, Error};
use serde_json::Value;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::exchanges::Exchange;
use crate::formats::xml;
use crate::time::{self, Date, DateTime, Period};
use crate::types::Decimal;
use crate::util;

use super::common::{send_request, parse_response};
use super::{QuotesProvider, SupportedExchange, QuotesMap, HistoricalQuotes};

const ENGINE: &str = "stock";
const MARKET: &str = "shares";

const STOCKS_BOARD: &str = "TQBR";
const ETF_BOARD: &str = "TQTF";

// API docs – https://www.moex.com/a2193
pub struct Moex {
    url: String,
    client: Client,
    incomplete_results_workaround: bool,
}

impl Moex {
    pub fn new(url: &str, incomplete_results_workaround: bool) -> Moex {
        Moex {
            url: url.to_owned(),
            client: Client::new(),
            incomplete_results_workaround,
        }
    }

    fn get_instrument_info(&self, symbol: &str) -> GenericResult<BTreeMap<Period, Board>> {
        let url = Url::parse_with_params(
            &format!("{}/iss/securities/{symbol}.json", self.url), &[
            ("iss.only", "boards"),
            ("boards.columns", "boardid,market,engine,history_from,history_till,currencyid"),

            ("iss.meta", "off"),
            ("iss.json", "extended"),
        ])?;

        Ok(send_request(&self.client, &url, None).and_then(|response| {
            parse_instrument_info(symbol, response)
        }).map_err(|e| format!("Failed to get instrument info from {url}: {e}"))?)
    }
}

impl QuotesProvider for Moex {
    fn name(&self) -> &'static str {
        "Moscow Exchange"
    }

    fn supports_stocks(&self) -> SupportedExchange {
        SupportedExchange::Some(Exchange::Moex)
    }

    fn supports_historical_stocks(&self) -> SupportedExchange {
        self.supports_stocks()
    }

    fn get_quotes(&self, symbols: &[&str]) -> GenericResult<QuotesMap> {
        let mut all_quotes = QuotesMap::new();
        let mut symbols: HashSet<&str> = symbols.iter().copied().collect();

        for board in [ETF_BOARD, STOCKS_BOARD] {
            let url = Url::parse_with_params(
                &format!("{}/iss/engines/{ENGINE}/markets/{MARKET}/boards/{board}/securities.xml", self.url),
                &[("securities", symbols.iter().sorted().join(",").as_str())],
            )?;

            let quotes = send_request(&self.client, &url, None).and_then(parse_quotes).map_err(|e| format!(
                "Failed to get quotes from {url}: {e}"))?;

            for symbol in quotes.keys() {
                symbols.remove(symbol.as_str());
            }
            all_quotes.extend(quotes);

            if symbols.is_empty() {
                break;
            }
        }

        Ok(all_quotes)
    }

    fn get_historical_quotes(&self, symbol: &str, period: Period) -> GenericResult<Option<HistoricalQuotes>> {
        let boards = self.get_instrument_info(symbol)?.into_iter().filter_map(|(board_period, board)| {
            period.try_intersect(board_period).map(|period| (period, board))
        }).collect_vec();

        if boards.is_empty() {
            return Ok(None);
        }

        let mut quotes = HistoricalQuotes::new();

        for (board_period, board) in boards {
            let mut start = 0;
            let mut tries = 0;
            let mut board_quotes: BTreeMap<Date, Vec<Decimal>> = BTreeMap::new();

            loop {
                let start_arg = start.to_string();
                let url = Url::parse_with_params(
                    &format!("{}/iss/engines/{}/markets/{}/boards/{}/securities/{symbol}/candles.json",
                        self.url, board.engine, board.market, board.name), &[

                    ("from", board_period.first_date().format("%Y.%m.%d").to_string().as_str()),
                    ("till", board_period.last_date().format("%Y.%m.%d").to_string().as_str()),
                    ("interval", "60"),
                    ("start", &start_arg),

                    ("candles.columns", "begin,open,close"),
                    ("iss.meta", "off"),
                ])?;

                tries += 1;
                let count = send_request(&self.client, &url, None).and_then(|response| {
                    parse_historical_quotes(response, &mut board_quotes)
                }).map_err(|e| format!("Failed to get historical quotes from {url}: {e}"))?;

                // The API is buggy: it may return incomplete results without any sign of it. So we have to workaround it
                // in a such ugly manner.
                if self.incomplete_results_workaround && count < 500 && tries < 60 && !board_quotes.last_key_value().map(|(&date, _)| {
                    date >= Exchange::Moex.min_last_working_day(board_period.last_date())
                }).unwrap_or_default() {
                    debug!("Looks like we've got incomplete results from MOEX historical API. Retrying...");
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }

                if count == 0 {
                    quotes.extend(super::aggregate_historical_quotes(&board.currency, board_quotes));
                    break;
                }

                start += count;
                tries = 0;
            }
        }

        Ok(Some(quotes))
    }
}

struct Board {
    name: String,
    market: String,
    engine: String,
    currency: String,
}

fn parse_instrument_info(symbol: &str, response: Response) -> GenericResult<BTreeMap<Period, Board>> {
    #[derive(Deserialize)]
    struct Response {
        boards: Vec<BoardInfo>,
    }

    #[derive(Deserialize)]
    struct BoardInfo {
        #[serde(rename = "boardid")]
        name: String,
        #[serde(rename = "engine")]
        engine: String,
        #[serde(rename = "market")]
        market: String,
        #[serde(rename = "history_from", deserialize_with = "deserialize_optional_date")]
        from: Option<Date>,
        #[serde(rename = "history_till", deserialize_with = "deserialize_optional_date")]
        till: Option<Date>,
        #[serde(rename = "currencyid")]
        currency: Option<String>,
    }

    let body = response.text()?;

    let mut response: Vec<Value> = parse_response(&body)?;
    if response.len() != 2 {
        return Err!("Got an unexpected response: {body}");
    }

    let response: Response = serde_json::from_value(response.pop().unwrap()).map_err(|e| format!(
        "Got an unexpected response: {e}"))?;

    let boards = response.boards.into_iter().filter(|board| {
        matches!(
            (board.name.as_str(), board.engine.as_str(), board.market.as_str()),
            (STOCKS_BOARD | ETF_BOARD, ENGINE, MARKET),
        ) && board.from.is_some() && board.till.is_some() && board.currency.is_some()
    }).collect_vec();

    let mut periods: BTreeMap<Period, Board> = BTreeMap::new();

    for board in boards {
        let period = Period::new(board.from.unwrap(), board.till.unwrap())?;

        for (&other_period, other_board) in &periods {
            if period.try_intersect(other_period).is_some() {
                return Err!("{} board ({period}) intersects with {} board ({other_period})", board.name, other_board.name);
            }
        }

        periods.insert(period, Board {
            name: board.name,
            market: board.market,
            engine: board.engine,
            currency: board.currency.unwrap(),
        });
    }

    if !periods.is_empty() {
        debug!("Got the following boards for {symbol}:{}", periods.iter().map(|(period, board)| {
            format!("\n* {period}: {}", board.name)
        }).join(""));
    }

    Ok(periods)
}

fn parse_quotes(response: Response) -> GenericResult<HashMap<String, Cash>> {
    #[derive(Deserialize)]
    struct Document {
        data: Vec<Data>,
    }

    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "@id")]
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
        // Common fields

        #[serde(rename = "@SECID")]
        symbol: Option<String>,

        // Security fields

        #[serde(rename = "@CURRENCYID")]
        currency: Option<String>,

        /// Previous trade day date
        #[serde(rename = "@PREVDATE")]
        prev_date: Option<String>,

        /// Previous trade day close price
        #[serde(rename = "@PREVLEGALCLOSEPRICE")]
        prev_price: Option<Decimal>,

        // Market data fields

        #[serde(rename = "@NUMTRADES")]
        trades: Option<u64>,

        #[serde(default, rename = "@LAST", deserialize_with = "deserialize_optional_decimal")]
        price: Option<Decimal>,

        // Time columns behaviour:
        // * 10.11.2018 closed session: UPDATETIME="19:18:26" TIME="18:41:07" SYSTIME="2018-11-09 19:33:27"
        // * 13.11.2018 open session: UPDATETIME="13:00:50" TIME="13:00:30" SYSTIME="2018-11-13 13:15:50"
        //
        // TIME - last trade time
        // UPDATETIME - data update time
        // SYSTIME - data fetch time
        #[serde(rename = "@SYSTIME")]
        time: Option<String>,
    }

    let result: Document = xml::deserialize(&response.bytes()? as &[u8])?;
    let (mut securities, mut market_data) = (None, None);

    for data in result.data {
        let data_ref = match data.id.as_str() {
            "securities" => &mut securities,
            "marketdata" => &mut market_data,
            _ => continue,
        };

        if data_ref.replace(data.table.rows).is_some() {
            return Err!("Duplicated {:?} data", data.id);
        }
    }

    let (securities, market_data) = match (securities, market_data) {
        (Some(securities), Some(market_data)) => (securities, market_data),
        _ => return Err!("Unable to find securities info in server response"),
    };

    let mut symbols = HashMap::new();

    for row in securities {
        let symbol = get_value(row.symbol)?;
        let currency = get_value(row.currency)?;
        let prev_date = get_value(row.prev_date)?;
        let prev_price = get_value(row.prev_price)?;

        let currency = match currency.as_str() {
            "SUR" => "RUB",
            _ => return Err!("{} is nominated in an unsupported currency: {}", symbol, currency),
        };

        let prev_date = time::parse_date(&prev_date, "%Y-%m-%d")?;
        if prev_price.is_zero() || prev_price.is_sign_negative() {
            return Err!("Invalid price: {}", prev_price);
        }

        if symbols.insert(symbol.clone(), (currency, prev_date, prev_price)).is_some() {
            return Err!("Duplicated symbol: {}", symbol);
        }
    }

    let mut quotes = HashMap::new();
    let mut outdated = Vec::new();

    for row in market_data {
        let symbol = get_value(row.symbol)?;

        let date = parse_time(&get_value(row.time)?)?.date();
        if is_outdated(date) {
            outdated.push(symbol);
            continue;
        }

        let trades = get_value(row.trades)?;
        let &(currency, prev_date, prev_price) = symbols.get(&symbol).ok_or_else(|| format!(
            "There is market data for {symbol} but security info is missing"))?;

        let price = match row.price {
            Some(price) => {
                if price.is_zero() || price.is_sign_negative() {
                    return Err!("Invalid price: {}", price);
                }

                price
            },
            None => {
                if trades != 0 {
                    return Err!("There is no last price for {}", symbol);
                }

                if is_outdated(prev_date) {
                    outdated.push(symbol);
                    continue;
                }

                prev_price
            },
        };

        if quotes.insert(symbol.clone(), Cash::new(currency, price)).is_some() {
            return Err!("Duplicated symbol: {}", symbol);
        }
    }

    if !outdated.is_empty() {
        error!("Got outdated quotes for the following symbols: {}.", outdated.join(", "));
    }

    Ok(quotes)
}

fn parse_historical_quotes(response: Response, quotes: &mut BTreeMap<Date, Vec<Decimal>>) -> GenericResult<usize> {
    #[derive(Deserialize)]
    struct Response {
        candles: Candles,
    }

    #[derive(Deserialize)]
    struct Candles {
        data: Vec<[Value;3]>,
    }

    let response: Response = parse_response(&response.text()?)?;
    let parse_price = |data: &Value| {
        data.as_f64()
            .and_then(Decimal::from_f64)
            .and_then(|price| util::validate_decimal(price, util::DecimalRestrictions::StrictlyPositive).ok())
    };

    for data in &response.candles.data {
        let (Some(date), Some(open), Some(close)) = (
            data[0].as_str().and_then(|time| parse_time(time).ok()).map(|time| time.date()),
            parse_price(&data[1]),
            parse_price(&data[2]),
        ) else {
            return Err!("Got an invalid candle: {data:?}");
        };

        let price = ((open + close) / dec!(2)).normalize();
        quotes.entry(date).or_default().push(price);
    }

    Ok(response.candles.data.len())
}

fn get_value<T>(value: Option<T>) -> GenericResult<T> {
    Ok(value.ok_or("Got an unexpected response from server")?)
}

fn parse_time(value: &str) -> GenericResult<DateTime> {
    time::parse_date_time(value, "%Y-%m-%d %H:%M:%S")
}

#[cfg(not(test))]
fn is_outdated(date: Date) -> bool {
    date < Exchange::Moex.min_last_working_day(time::today())
}

#[cfg(test)]
fn is_outdated(_date: Date) -> bool {
    false
}

fn deserialize_optional_date<'de, D>(deserializer: D) -> Result<Option<Date>, D::Error>
    where D: Deserializer<'de>
{
    let value: Option<String> = Deserialize::deserialize(deserializer)?;
    value.map(|date| time::parse_date(date.as_str(), "%Y-%m-%d"))
        .transpose().map_err(|e| D::Error::custom(e.to_string()))
}

fn deserialize_optional_decimal<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    if value.is_empty() {
        return Ok(None);
    }

    Ok(Some(Decimal::from_str(&value)
        .map_err(|_| D::Error::custom(format!("Invalid decimal value: {value:?}")))?))
}

#[cfg(test)]
mod tests {
    use std::borrow::ToOwned;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;

    use mockito::{Server, ServerGuard, Mock};

    use super::*;

    #[test]
    fn missing_instrument_info() {
        let (mut server, client) = create_server();
        let _mock = mock_instrument_info(&mut server, "INVALID", "moex-instrument-missing.json");
        assert!(client.get_instrument_info("INVALID").unwrap().is_empty());
    }

    #[test]
    fn instrument_info_single_board() {
        let (mut server, client) = create_server();
        let _mock = mock_instrument_info(&mut server, "FXUS", "moex-instrument-fxus.json");

        let boards = client.get_instrument_info("FXUS").unwrap().into_values().map(|board| board.name).collect_vec();
        assert_eq!(boards, vec![ETF_BOARD]);
    }

    #[test]
    fn instrument_info_multiple_boards() {
        let (mut server, client) = create_server();
        let _mock = mock_instrument_info(&mut server, "FXRB", "moex-instrument-fxrb.json");

        let boards = client.get_instrument_info("FXRB").unwrap().into_values().map(|board| board.name).collect_vec();
        assert_eq!(boards, vec![STOCKS_BOARD, ETF_BOARD]);
    }

    #[test]
    fn quotes() {
        let (mut server, client) = create_server();
        let _mock = mock_quotes(&mut server, ETF_BOARD, &["FXUS", "FXIT", "INVALID"], "moex.xml");
        let _mock = mock_quotes(&mut server, STOCKS_BOARD, &["INVALID"], "moex-empty.xml");

        let mut quotes = HashMap::new();
        quotes.insert(s!("FXUS"), Cash::new("RUB", dec!(3320)));
        quotes.insert(s!("FXIT"), Cash::new("RUB", dec!(4612)));

        assert_eq!(client.get_quotes(&["FXUS", "FXIT", "INVALID"]).unwrap(), quotes);
    }

    #[test]
    fn exchange_closed_quotes() {
        test_exchange_status("closed")
    }

    #[test]
    fn exchange_opening_quotes() {
        test_exchange_status("opening")
    }

    #[test]
    fn exchange_open_quotes() {
        test_exchange_status("open")
    }

    fn test_exchange_status(status: &str) {
        let securities = ["FXAU", "FXCN", "FXDE", "FXIT", "FXJP", "FXRB", "FXRL", "FXRU", "FXUK", "FXUS"];

        let (mut server, client) = create_server();
        let _mock = mock_quotes(&mut server, ETF_BOARD, &securities, &format!("moex-{status}.xml"));

        let quotes = client.get_quotes(&securities).unwrap();
        assert_eq!(
            quotes.keys().map(String::as_str).collect::<HashSet<&str>>(),
            securities.iter().cloned().collect::<HashSet<&str>>(),
        );
    }

    #[test]
    fn historical_quotes() {
        let (mut server, client) = create_server();

        let _mock = mock_instrument_info(&mut server, "FXUS", "moex-instrument-fxus.json");
        let _mock = mock_candles(&mut server, 0, "moex-historical-fxus-start.json");
        let _mock = mock_candles(&mut server, 4, "moex-historical-fxus-end.json");

        let period = Period::new(date!(2016, 5, 25), date!(2016, 5, 30)).unwrap();
        let quotes = client.get_historical_quotes("FXUS", period).unwrap().unwrap();

        assert_eq!(quotes, btreemap! {
            date!(2016, 5, 25) => Cash::new("RUB", dec!(24.275)),
            date!(2016, 5, 26) => Cash::new("RUB", dec!(23.98)),
            date!(2016, 5, 27) => Cash::new("RUB", dec!(24.24)),
            date!(2016, 5, 30) => Cash::new("RUB", dec!(24.275)),
        });
    }

    fn create_server() -> (ServerGuard, Moex) {
        let server = Server::new();
        let client = Moex::new(&server.url(), false);
        (server, client)
    }

    fn mock_instrument_info(server: &mut Server, symbol: &str, body_path: &str) -> Mock {
        let path = format!("/iss/securities/{symbol}.json?iss.only=boards&boards.columns=boardid%2Cmarket%2Cengine%2Chistory_from%2Chistory_till%2Ccurrencyid&iss.meta=off&iss.json=extended");
        mock_response(server, &path, body_path)
    }

    fn mock_quotes(server: &mut Server, board: &str, securities: &[&str], body_path: &str) -> Mock {
        let securities =
            url::form_urlencoded::byte_serialize(securities.iter().copied().sorted().join(",").as_bytes())
            .collect::<String>();

        let path = format!("/iss/engines/{ENGINE}/markets/{MARKET}/boards/{board}/securities.xml?securities={securities}");
        mock_response(server, &path, body_path)
    }

    fn mock_candles(server: &mut Server, start: usize, body_path: &str) -> Mock {
        let path = format!("/iss/engines/{ENGINE}/markets/{MARKET}/boards/{ETF_BOARD}/securities/FXUS/candles.json?from=2016.05.25&till=2016.05.30&interval=60&start={start}&candles.columns=begin%2Copen%2Cclose&iss.meta=off");
        mock_response(server, &path, body_path)
    }

    fn mock_response(server: &mut Server, path: &str, body_path: &str) -> Mock {
        let mut body = String::new();
        let body_path = Path::new(file!()).parent().unwrap().join("testdata").join(body_path);
        File::open(body_path).unwrap().read_to_string(&mut body).unwrap();

        server.mock("GET", path)
            .with_status(200)
            .with_body(body)
            .create()
    }
}