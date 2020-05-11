use chrono::Duration;
use lazy_static::lazy_static;
use regex::Regex;

use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::{EmptyResult, GenericResult};
use crate::formatting;
use crate::types::Date;
use crate::xls;

use super::common::parse_date;

pub struct PeriodParser {
}

impl SectionParser for PeriodParser {
    fn consume_title(&self) -> bool { false }

    fn parse(&self, parser: &mut XlsStatementParser) -> EmptyResult {
        let row = xls::strip_row_expecting_columns(parser.sheet.next_row_checked()?, 1)?;
        let period = parse_period(xls::get_string_cell(row[0])?)?;
        parser.statement.set_period(period)?;
        parser.statement.set_starting_assets(false)?; // FIXME(konishchev): A temporary hack
        parser.statement.cash_assets.deposit(crate::currency::Cash::new("RUB", dec!(0))); // FIXME(konishchev): A temporary hack
        Ok(())
    }
}

fn parse_period(value: &str) -> GenericResult<(Date, Date)> {
    lazy_static! {
        static ref PERIOD_REGEX: Regex = Regex::new(
            r"^Отчет о сделках и операциях за период (?P<start>\d{2}\.\d{2}\.\d{4}) - (?P<end>\d{2}\.\d{2}\.\d{4})$").unwrap();
    }

    let captures = PERIOD_REGEX.captures(value).ok_or_else(|| format!(
        "Invalid period: {:?}", value))?;
    let start = parse_date(captures.name("start").unwrap().as_str())?;
    let end = parse_date(captures.name("end").unwrap().as_str())? + Duration::days(1);

    if start >= end {
        return Err!("Invalid period: {}", formatting::format_period(start, end));
    }

    Ok((start, end))
}