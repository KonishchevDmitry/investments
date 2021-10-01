use lazy_static::lazy_static;
use regex::Regex;

use crate::broker_statement::partial::PartialBrokerStatementRc;
use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::{EmptyResult, GenericResult};
use crate::time;
use crate::types::Date;
use crate::xls;

use super::common::parse_date;

pub struct PeriodParser {
    statement: PartialBrokerStatementRc,
}

impl PeriodParser {
    pub fn new(statement: PartialBrokerStatementRc) -> Box<dyn SectionParser> {
        Box::new(PeriodParser {statement})
    }
}

impl SectionParser for PeriodParser {
    fn consume_title(&self) -> bool { false }

    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let row = xls::strip_row_expecting_columns(parser.sheet.next_row_checked()?, 2)?;
        let period = parse_period(xls::get_string_cell(row[1])?)?;
        self.statement.borrow_mut().set_period(period)?;
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

    time::parse_period(
        parse_date(captures.name("start").unwrap().as_str())?,
        parse_date(captures.name("end").unwrap().as_str())?,
    )
}