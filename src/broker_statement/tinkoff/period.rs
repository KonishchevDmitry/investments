use lazy_static::lazy_static;
use regex::{self, Regex};

use crate::broker_statement::partial::PartialBrokerStatementRc;
use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::{EmptyResult, GenericResult};
use crate::formatting;
use crate::time;
use crate::types::Date;
use crate::xls;

use super::common::parse_date;

pub struct PeriodParser {
    statement: PartialBrokerStatementRc,
    calculation_date: Option<Date>,
}

impl PeriodParser {
    pub const CALCULATION_DATE_PREFIX: &'static str = "Дата расчета: ";
    pub const PERIOD_PREFIX: &'static str = "Отчет о сделках и операциях за период ";

    pub fn new(statement: PartialBrokerStatementRc) -> Box<dyn SectionParser> {
        Box::new(PeriodParser {
            statement: statement,
            calculation_date: None,
        })
    }
}

impl SectionParser for PeriodParser {
    fn consume_title(&self) -> bool { false }

    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let row = xls::strip_row_expecting_columns(parser.sheet.next_row_checked()?, 1)?;
        let cell = xls::get_string_cell(row[0])?;

        if let Some(date) = cell.strip_prefix(PeriodParser::CALCULATION_DATE_PREFIX) {
            let calculation_date = parse_date(date.trim())?;
            if self.calculation_date.replace(calculation_date).is_some() {
                return Err!("Got a duplicated statement creation date")
            }
        } else if let Some(date) = cell.strip_prefix(PeriodParser::PERIOD_PREFIX) {
            let calculation_date = self.calculation_date.ok_or(
                "Got statement period without calculation date")?;

            let mut period = parse_period(date.trim())?;
            period.1 = std::cmp::min(period.1, calculation_date.succ());
            if period.1 <= period.0 {
                return Err!("Got an invalid statement period: {}", formatting::format_period(period));
            }

            self.statement.borrow_mut().set_period(period)?;
        } else {
            return Err!("Got an unexpected cell value: {:?}", cell);
        }

        Ok(())
    }
}

fn parse_period(value: &str) -> GenericResult<(Date, Date)> {
    lazy_static! {
        static ref PERIOD_REGEX: Regex = Regex::new(
            r"^(?P<start>\d{2}\.\d{2}\.\d{4}) - (?P<end>\d{2}\.\d{2}\.\d{4})$").unwrap();
    }

    let captures = PERIOD_REGEX.captures(value).ok_or_else(|| format!(
        "Invalid period: {:?}", value))?;

    time::parse_period(
        parse_date(captures.name("start").unwrap().as_str())?,
        parse_date(captures.name("end").unwrap().as_str())?,
    )
}