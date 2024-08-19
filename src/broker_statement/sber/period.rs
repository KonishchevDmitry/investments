use lazy_static::lazy_static;
use scraper::ElementRef;
use regex::Regex;

use crate::broker_statement::partial::PartialBrokerStatementRc;
use crate::core::EmptyResult;
use crate::formats::html::{self, SectionParser, SectionType};
use crate::time::Period;

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
    fn section_type(&self) -> SectionType {
        SectionType::Simple
    }

    fn parse(&mut self, element: ElementRef) -> EmptyResult {
        let text = html::textify(element);

        lazy_static! {
            static ref PERIOD_REGEX: Regex = Regex::new(
                r"^Отчет брокера за период с (?P<start>\d{2}\.\d{2}\.\d{4}) по (?P<end>\d{2}\.\d{2}\.\d{4})").unwrap();
        }

        let captures = PERIOD_REGEX.captures(&text).ok_or_else(|| format!(
            "Unable to parse broker statement period from the following string: {text:?}"))?;

        self.statement.borrow_mut().set_period(Period::new(
            parse_date(captures.name("start").unwrap().as_str())?,
            parse_date(captures.name("end").unwrap().as_str())?,
        )?)
    }
}