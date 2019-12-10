use chrono::Duration;
use lazy_static::lazy_static;
use log::trace;
use regex::Regex;

use crate::core::{EmptyResult, GenericResult};
use crate::formatting;
use crate::types::Date;
use crate::xls;

use super::{Parser, SectionParser};
use super::common::parse_date;

pub struct PeriodParser {
}

impl SectionParser for PeriodParser {
    fn consume_title(&self) -> bool { false }

    fn parse(&self, parser: &mut Parser) -> EmptyResult {
        let row = xls::strip_row_expecting_columns(parser.sheet.next_row_checked()?, 2)?;
        let period = parse_period(xls::get_string_cell(row[1])?)?;
        parser.statement.set_period(period)?;
        Ok(())
    }
}

fn parse_period(value: &str) -> GenericResult<(Date, Date)> {
    lazy_static! {
        static ref PERIOD_REGEX: Regex = Regex::new(
            r"^с (?P<start>\d{2}\.\d{2}\.\d{4}) по (?P<end>\d{2}\.\d{2}\.\d{4})$").unwrap();
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

pub struct AssetsParser {
}

impl SectionParser for AssetsParser {
    // FIXME
    fn parse(&self, parser: &mut Parser) -> EmptyResult {
        parser.sheet.skip_empty_rows();
        parser.sheet.skip_non_empty_rows();
        parser.sheet.skip_empty_rows();
        parser.sheet.next_row_checked()?;

        let columns = &[
        "Вид актива",
        "Номер гос. регистрации ЦБ/ ISIN",
        "Тип ЦБ (№ вып.)",
        "Кол-во ценных бумаг",
        "Цена закрытия/котировка вторич.(5*)",
        "Сумма НКД",
        "Сумма, в т.ч. НКД",
        "Кол-во ценных бумаг",
        "Цена закрытия/ котировка вторич.(5*)",
        "Сумма НКД",
        "Сумма, в т.ч. НКД",
        "Организатор торгов (2*)",
        "Место хранения",
        "Эмитент",
        ];
        let row = parser.sheet.next_row_checked()?;
        trace!("{:?}", row);
        let columns_mapping = xls::map_columns(row, columns)?;

        let row = parser.sheet.next_row_checked()?;
        trace!("{:?}", row);
        columns_mapping.map(row)?;

        let row = parser.sheet.next_row_checked()?;
        trace!("{:?}", row);
        columns_mapping.map(row)?;

        Ok(())
    }
}