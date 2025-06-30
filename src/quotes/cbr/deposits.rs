use std::borrow::Cow;
use std::collections::BTreeMap;
#[cfg(test)] use std::fs;
use std::io::Cursor;
#[cfg(test)] use std::path::Path;

use calamine::{Reader, Xlsx};
use chrono::Months;
use itertools::Itertools;
use log::{debug, warn};
use reqwest::blocking::{Client, Response};

use crate::core::{EmptyResult, GenericResult};
use crate::formats::xls::{self, XlsTableRow, SheetReader, SheetParser, TableReader, RawRowType, Cell, SkipCell};
use crate::quotes::common::send_request;
use crate::time::{self, Date};
use crate::types::Decimal;

// Статистика -> Банковский сектор -> Процентные ставки по кредитам и депозитам и структура кредитов и депозитов по
// срочности -> Сведения по вкладам (депозитам) физических лиц и нефинансовых организаций в рублях, долларах США и евро
// (https://www.cbr.ru/statistics/bank_sector/int_rat/)
pub fn get_interest_rates(base_url: &str) -> GenericResult<Vec<DepositStatistics>> {
    let client = Client::new();
    let url = format!("{}/vfs/statistics/pdko/int_rat/deposits.xlsx", base_url);

    Ok(send_request(&client, &url, None)
        .and_then(parse_interest_rates)
        .map_err(|e| format!("Failed to get deposit interest rates from {url}: {e}"))?)
}

pub struct DepositStatistics {
    pub name: String,
    pub currency: &'static str,
    pub duration: u32,
    pub interest_rates: BTreeMap<i32, Vec<Decimal>>,
}

impl DepositStatistics {
    fn new(name: &str, currency: &'static str, duration: u32) -> DepositStatistics {
        DepositStatistics {
            name: name.to_owned(),
            currency,
            duration,
            interest_rates: BTreeMap::new(),
        }
    }

    fn add(&mut self, year: i32, month: u32, interest: Decimal) -> EmptyResult {
        if let Some(rates) = self.interest_rates.get_mut(&year) {
            if month as usize != rates.len() + 1 {
                let (&last_year, last_rates) = self.interest_rates.last_key_value().unwrap();
                return Err!("Got {year}.{month:02} interest rates after {last_year}.{:02} interest rates", last_rates.len());
            }
            rates.push(interest);
            return Ok(());
        }

        if let Some((&last_year, last_rates)) = self.interest_rates.last_key_value() {
            if year != last_year + 1 || last_rates.len() != 12 || month != 1 {
                return Err!("Got {year}.{month:02} interest rates after {last_year}.{:02} interest rates", last_rates.len());
            }
        } else if month != 1 {
            return Err!("Got {year}.{month:02} interest rates having no interest rates for the previous months");
        }

        let mut rates = Vec::with_capacity(12);
        rates.push(interest);
        self.interest_rates.insert(year, rates);

        Ok(())
    }
}

struct Parser {
    currency: &'static str,
    sheet_name: &'static str,
}

impl Parser {
    fn new(currency: &'static str, sheet_name: &'static str) -> Parser {
        Parser {
            currency,
            sheet_name,
        }
    }
}

impl SheetParser for Parser {
    fn sheet_name(&self) -> &str {
        self.sheet_name
    }
}

#[derive(XlsTableRow)]
#[table(trim_column_title = "trim_column_title", skip_row = "skip_row")]
struct InterestRatesRow {
    #[column(name = "")]
    date: String,

    // Физических лиц, со сроком привлечения
    #[column(name = "''до востребова-ния''")]
    _1: SkipCell,
    #[column(name = "до 30 дней, включая ''до востребова-ния''")]
    _2: SkipCell,
    #[column(name = "до 30 дней, кроме ''до востребова-ния''")]
    _3: SkipCell,
    #[column(name = "от 31 до 90 дней")]
    _4: SkipCell,
    #[column(name = "от 91 до 180 дней")]
    interest_3m: Decimal,
    #[column(name = "от 181 дня до 1 года")]
    interest_6m: Decimal,
    #[column(name = "до 1 года, включая ''до востребова-ния''")]
    _7: SkipCell,
    #[column(name = "до 1 года, кроме ''до востребова-ния''")]
    _8: SkipCell,
    #[column(name = "от 1 года до 3 лет")]
    interest_1y: Decimal,
    #[column(name = "свыше 3 лет")]
    _10: SkipCell,
    #[column(name = "свыше 1 года")]
    _11: SkipCell,

    // Нефинансовых организаций, со сроком привлечения"
    #[column(name = "до 30 дней, включая ''до востребова-ния''")]
    _12: SkipCell,
    #[column(name = "от 31 до 90 дней")]
    _13: SkipCell,
    #[column(name = "от 91 до 180 дней")]
    _14: SkipCell,
    #[column(name = "от 181 дня до 1 года")]
    _15: SkipCell,
    #[column(name = "до 1 года, включая ''до востребова-ния''")]
    _16: SkipCell,
    #[column(name = "от 1 года до 3 лет")]
    _17: SkipCell,
    #[column(name = "свыше 3 лет")]
    _18: SkipCell,
    #[column(name = "свыше 1 года")]
    _19: SkipCell,
}

impl TableReader for InterestRatesRow {
}

fn trim_column_title(title: &str) -> Cow<str> {
    Cow::from(title.trim_end_matches('*')) // Footnotes
}

fn skip_row(row: RawRowType) -> bool {
    xls::trim_row(row).len() < 2 // Footnotes
}

fn parse_interest_rates(response: Response) -> GenericResult<Vec<DepositStatistics>> {
    let data = response.bytes()?;

    let mut document = Xlsx::new(Cursor::new(data))?;
    let mut statistics = Vec::new();

    for parser in [
        Parser::new("RUB", "ставки_руб"),
        Parser::new("USD", "ставки_долл.США"),
    ] {
        let currency = parser.currency;
        let sheet_name = parser.sheet_name().to_owned();

        let sheet = document.worksheet_range(&sheet_name)?;
        let reader = SheetReader::new(sheet, Box::new(parser));

        statistics.extend(parse_interest_rates_sheet(currency, reader).map_err(|e| format!(
            "Failed to parse {sheet_name:?} sheet: {e}"))?);
    }

    Ok(statistics)
}

fn parse_interest_rates_sheet(currency: &'static str, mut reader: SheetReader) -> GenericResult<[DepositStatistics; 3]> {
    loop {
        let row = reader.next_row().ok_or(
            "Unable to find interest rates table")?;

        let trimmed_row = row.iter().filter_map(|cell| {
            match cell {
                Cell::String(value) => if value.is_empty() {
                    None
                } else {
                    Some(value.as_str())
                },
                Cell::Empty => None,
                _ => Some(""),
            }
        });

        if trimmed_row.collect_array() == Some([
            "Физических лиц, со сроком привлечения",
            "Нефинансовых организаций, со сроком привлечения",
        ]) {
            break;
        }
    }

    let mut deposit_3m = DepositStatistics::new(&format!("{currency} deposit: 3 months"), currency, 91);
    let mut deposit_6m = DepositStatistics::new(&format!("{currency} deposit: 6 months"), currency, 181);
    let mut deposit_1y = DepositStatistics::new(&format!("{currency} deposit: 1 year"), currency, 365);

    for row in xls::read_table::<InterestRatesRow>(&mut reader)? {
        let (year, month) = parse_date(&row.date).ok_or_else(|| format!(
            "Got an invalid date: {:?}", row.date))?;

        deposit_3m.add(year, month, row.interest_3m)?;
        deposit_6m.add(year, month, row.interest_6m)?;
        deposit_1y.add(year, month, row.interest_1y)?;
    }

    let (
        Some((&first_year, first_rates)),
        Some((&last_year, last_rates)),
    ) = (
        deposit_1y.interest_rates.first_key_value(),
        deposit_1y.interest_rates.last_key_value(),
    ) else {
        return Err!("Got an empty interest rates table");
    };

    let first_human_date = format!("{first_year}.{:02}", first_rates.len());
    let last_human_date = format!("{last_year}.{:02}", last_rates.len());

    let last_date = Date::from_ymd_opt(last_year, last_rates.len() as u32, 1)
        .and_then(|date| date.checked_add_months(Months::new(1)))
        .and_then(|date| date.pred_opt())
        .ok_or_else(|| format!("Got an invalid date: {last_human_date}"))?;

    if (time::today() - last_date).num_days() < 61 {
        debug!("Got {currency} deposit interest rates for {first_human_date} - {last_human_date}.")
    } else {
        warn!("Got an outdated {currency} deposit interest rates: {first_human_date} - {last_human_date}.")
    }

    Ok([deposit_3m, deposit_6m, deposit_1y])
}

fn parse_date(name: &str) -> Option<(i32, u32)> {
    let (month, year) = name.split(' ').collect_tuple()?;

    let year = year.parse().ok()?;
    let month = match month {
        "Январь" => 1,
        "Февраль" => 2,
        "Март" => 3,
        "Апрель" => 4,
        "Май" => 5,
        "Июнь" => 6,
        "Июль" => 7,
        "Август" => 8,
        "Сентябрь" => 9,
        "Октябрь" => 10,
        "Ноябрь" => 11,
        "Декабрь" => 12,
        _ => return None,
    };

    Some((year, month))
}

#[cfg(test)]
mod tests {
    use mockito::{Server, Mock};
    use super::*;

    #[test]
    fn interest_rates() {
        let mut server = Server::new();
        let _mock = mock_response(&mut server);

        let deposits = get_interest_rates(&server.url()).unwrap().into_iter().map(|deposit| {
            (deposit.currency, deposit.duration, deposit.interest_rates[&2020][2])
        }).collect_vec();

        assert_eq!(deposits, vec![
            ("RUB", 91, dec!(4.37)),
            ("RUB", 181, dec!(4.62)),
            ("RUB", 365, dec!(4.91)),

            ("USD", 91, dec!(0.39)),
            ("USD", 181, dec!(0.84)),
            ("USD", 365, dec!(0.87)),
        ]);
    }

    fn mock_response(server: &mut Server) -> Mock {
        let path = Path::new(file!()).parent().unwrap().join("testdata/deposits.xlsx");

        server.mock("GET", "/vfs/statistics/pdko/int_rat/deposits.xlsx")
            .with_status(200)
            .with_header("Content-Type", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
            .with_body(fs::read(path).unwrap())
            .create()
    }
}