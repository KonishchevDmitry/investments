use std::iter::Iterator;
use std::str::FromStr;

use csv::StringRecord;

use broker_statement::ib::IbStatementParser;
use core::{EmptyResult, GenericResult};
use types::Date;
use util;

pub struct Record<'a> {
    pub name: &'a str,
    pub fields: &'a Vec<&'a str>,
    pub values: &'a StringRecord,
}

impl<'a> Record<'a> {
    pub fn get_value(&self, field: &str) -> GenericResult<&str> {
        if let Some(index) = self.fields.iter().position(|other: &&str| *other == field) {
            if let Some(value) = self.values.get(index + 2) {
                return Ok(value);
            }
        }

        Err!("{:?} record doesn't have {:?} field", self.name, field)
    }

    pub fn parse_value<T: FromStr>(&self, field: &str) -> GenericResult<T> {
        let value = self.get_value(field)?;
        Ok(value.parse().map_err(|_| format!(
            "{:?} field has an invalid value: {:?}", field, value))?)
    }
}

pub trait RecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> { Some(&["Data"]) }
    fn skip_data_types(&self) -> Option<&'static [&'static str]> { None }
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult;
}

pub fn format_record<'a, I>(iter: I) -> String
    where I: IntoIterator<Item = &'a str> {

    iter.into_iter()
        .map(|value| format!("{:?}", value))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn parse_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%Y-%m-%d")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("2018-06-22").unwrap(), date!(22, 6, 2018));
    }
}