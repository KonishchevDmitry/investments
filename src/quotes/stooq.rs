#[cfg(test)] use indoc::indoc;
use reqwest::Url;
use reqwest::blocking::{Client, Response};

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::exchanges::Exchange;
use crate::time::{Date, Period};
use crate::util::{self, DecimalRestrictions};

use super::{QuotesProvider, SupportedExchange, HistoricalQuotes};
use super::alphavantage::AlphaVantage;
#[cfg(test)] use super::alphavantage::AlphaVantageConfig;
use super::common::send_request;

pub struct Stooq {
    url: String,
    client: Client,
    symbols: AlphaVantage,
}

impl Stooq {
    pub fn new(url: &str, alphavantage: AlphaVantage) -> Stooq {
        Stooq {
            url: url.to_owned(),
            client: Client::new(),
            symbols: alphavantage,
        }
    }
}

impl QuotesProvider for Stooq {
    fn name(&self) -> &'static str {
        "Stooq"
    }

    // For now support only LSE, because historical quotes are needed only for backtesting in which we have to use
    // accumulating ETF only.
    fn supports_historical_stocks(&self) -> SupportedExchange {
        SupportedExchange::Some(Exchange::Lse)
    }

    fn get_historical_quotes(&self, symbol: &str, _period: Period) -> GenericResult<Option<HistoricalQuotes>> {
        // Stooq provides free historical quotes without any rate limiting, but doesn't provide their currency, so we
        // use Alpha Vantage to determine it.
        //
        // Alpha Vantage can't be used for historical quotes, because its free historical quotes are raw only (not
        // split-adjusted).

        let Some(currency) = self.symbols.find_symbol(symbol)?.remove(&format!("{symbol}.LON")) else {
            return Ok(None);
        };

        let url = Url::parse_with_params(&format!("{}/q/d/l/", self.url), &[
            ("s", format!("{symbol}.UK").as_str()),
            ("i", "d"),
        ])?;

        Ok(send_request(&self.client, &url, None).and_then(|response| {
            parse_historical_quotes(&currency, response)
        }).map_err(|e| format!("Failed to get historical quotes from {url}: {e}"))?)
    }
}

fn parse_historical_quotes(currency: &str, response: Response) -> GenericResult<Option<HistoricalQuotes>> {
    let response = response.text()?;
    if response.trim() == "No data" {
        return Ok(None);
    }

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(response.as_bytes());

    let headers = reader.headers()?;

    let (Some(date_index), Some(open_index), Some(close_index)) = (
        headers.iter().position(|name| name == "Date"),
        headers.iter().position(|name| name == "Open"),
        headers.iter().position(|name| name == "Close"),
    ) else {
        return Err!("Got an unexpected header: {:?}", headers);
    };

    let parse_date = |date: &str| Date::parse_from_str(date, "%Y-%m-%d").ok();
    let parse_price = |price: &str| util::parse_decimal(price, DecimalRestrictions::StrictlyPositive).ok();

    let mut quotes = HistoricalQuotes::new();

    for record in reader.records() {
        let record = record?;

        let (Some(date), Some(open), Some(close)) = (
            record.get(date_index).and_then(parse_date),
            record.get(open_index).and_then(parse_price),
            record.get(close_index).and_then(parse_price),
        ) else {
            return Err!("Got an unexpected record: {:?}", record);
        };

        let price = (open + close) / dec!(2);
        quotes.insert(date, Cash::new(currency, price).normalize_currency());
    }

    Ok(Some(quotes))
}

#[cfg(test)]
mod tests {
    use mockito::{Server, ServerGuard, Mock};
    use super::*;

    #[test]
    fn historical_quotes() {
        let (mut server, client) = create_server();

        let _mock = mock(&mut server, "/query?function=SYMBOL_SEARCH&keywords=SSAC&apikey=mock", indoc!(r#"
            {
                "bestMatches": [
                    {
                        "1. symbol": "SSAC.LON",
                        "2. name": "iShares MSCI ACWI UCITS ETF USD (Acc) GBP",
                        "3. type": "ETF",
                        "4. region": "United Kingdom",
                        "5. marketOpen": "08:00",
                        "6. marketClose": "16:30",
                        "7. timezone": "UTC+01",
                        "8. currency": "GBX",
                        "9. matchScore": "0.8000"
                    },
                    {
                        "1. symbol": "SSAC.AMS",
                        "2. name": "iShares MSCI ACWI UCITS ETF USD (Acc) EUR",
                        "3. type": "ETF",
                        "4. region": "Amsterdam",
                        "5. marketOpen": "09:00",
                        "6. marketClose": "17:40",
                        "7. timezone": "UTC+01",
                        "8. currency": "EUR",
                        "9. matchScore": "0.7273"
                    },
                    {
                        "1. symbol": "SSACD.PAR",
                        "2. name": "Euronext S Credit Agricole 070322 GR Decr 1.05",
                        "3. type": "Equity",
                        "4. region": "Paris",
                        "5. marketOpen": "09:00",
                        "6. marketClose": "17:30",
                        "7. timezone": "UTC+02",
                        "8. currency": "EUR",
                        "9. matchScore": "0.6667"
                    }
                ]
            }
        "#));

        let _mock = mock(&mut server, "/q/d/l/?s=SSAC.UK&i=d", indoc!(r#"
            Date,Open,High,Low,Close,Volume
            2025-04-03,6588,6603,6500,6531,81860
            2025-04-04,6492,6500,6248,6300,247645
            2025-04-07,5886,6272,5886,6141,271971
            2025-04-08,6264,6425,6264,6318.5,125741
            2025-04-09,6102,6200,6006,6115,51282
            2025-04-10,6603,6603,6349.5,6349.5,74558
            2025-04-11,6396,6396,6271,6320.5,25531
            2025-04-14,6461,6498,6442,6442,39136
            2025-04-15,6456,6470,6424,6451.5,43963
            2025-04-16,6349,6419,6329,6419,59396
            2025-04-17,6383,6386,6316,6352,336588
            2025-04-22,6298,6311,6236,6311,47250
            2025-04-23,6434,6530,6433,6474,23880
            2025-04-24,6443,6516,6424,6515,24328
            2025-04-25,6577,6577,6522,6539,47937
            2025-04-28,6581,6581,6521,6524,17953
            2025-04-29,6549,6571,6529,6571,20415
            2025-04-30,6582,6603,6531,6571,48541
            2025-05-01,6655,6719,6655,6714,93075
            2025-05-02,6720,6766,6713,6763,6959
            2025-05-06,6751,6751,6665,6718,24196
            2025-05-07,6714,6729,6680,6687,9205
            2025-05-08,6769,6796,6721,6749,30328
        "#));

        let period = Period::new(date!(2025, 4, 10), date!(2025, 4, 25)).unwrap();
        let quotes = client.get_historical_quotes("SSAC", period).unwrap().unwrap();

        assert_eq!(*quotes.first_key_value().unwrap().0, date!(2025, 4, 3));
        assert_eq!(*quotes.last_key_value().unwrap().0, date!(2025, 5, 8));
        assert_eq!(quotes[&date!(2025, 4, 22)], Cash::new("GBX", dec!(6304.5)));
    }

    #[test]
    fn historical_quotes_unknown() {
        let (mut server, client) = create_server();

        let _mock = mock(&mut server, "/query?function=SYMBOL_SEARCH&keywords=UNKNOWN&apikey=mock", indoc!(r#"
            {
                "bestMatches": [
                    {
                        "1. symbol": "UNKNOWN",
                        "8. currency": "MOCK"
                    }
                ]
            }
        "#));

        let _mock = mock(&mut server, "/q/d/l?s=UNKNOWN&i=d", "No data");

        let period = Period::new(date!(2025, 4, 10), date!(2025, 4, 25)).unwrap();
        assert_eq!(client.get_historical_quotes("UNKNOWN", period).unwrap(), None);
    }

    fn create_server() -> (ServerGuard, Stooq) {
        let server = Server::new();

        let client = Stooq::new(&server.url(), AlphaVantage::new(&AlphaVantageConfig {
             url: server.url().clone(),
             api_key: s!("mock"),
        }));

        (server, client)
    }

    fn mock(server: &mut Server, path: &str, data: &str) -> Mock {
        server.mock("GET", path)
            .with_status(200)
            .with_header("Content-Type", "text/plain")
            .with_body(data)
            .create()
    }
}