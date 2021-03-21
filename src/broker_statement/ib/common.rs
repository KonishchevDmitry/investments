use std::iter::Iterator;
use std::str::FromStr;

use csv::StringRecord;

use crate::broker_statement::ib::StatementParser;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::types::{Date, DateTime, Decimal};
use crate::util::{self, DecimalRestrictions};

pub struct RecordSpec<'a> {
    pub name: &'a str,
    fields: Vec<&'a str>,
    offset: usize,
}

impl<'a> RecordSpec<'a> {
    pub fn new(name: &'a str, fields: Vec<&'a str>, offset: usize) -> RecordSpec<'a> {
        RecordSpec {name, fields, offset}
    }
}

pub struct Record<'a> {
    pub spec: &'a RecordSpec<'a>,
    pub values: &'a StringRecord,
}

impl<'a> Record<'a> {
    pub fn new(spec: &'a RecordSpec<'a>, values: &'a StringRecord) -> Record<'a> {
        Record {spec, values}
    }

    pub fn get_value(&self, field: &str) -> GenericResult<&str> {
        if let Some(index) = self.spec.fields.iter().position(|other: &&str| *other == field) {
            if let Some(value) = self.values.get(self.spec.offset + index) {
                return Ok(value);
            }
        }

        Err!("{:?} record doesn't have {:?} field", self.spec.name, field)
    }

    pub fn check_value(&self, field: &str, value: &str) -> EmptyResult {
        self.check_values(&[(field, value)])
    }

    pub fn check_values(&self, values: &[(&str, &str)]) -> EmptyResult {
        for (field, value) in values.iter() {
            if self.get_value(*field)? != *value {
                return Err!("Got an unexpected {:?} field value: {:?}", *field, *value);
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn parse_value<T: FromStr>(&self, field: &str) -> GenericResult<T> {
        let value = self.get_value(field)?;
        Ok(value.parse().map_err(|_| format!(
            "{:?} field has an invalid value: {:?}", field, value))?)
    }

    pub fn parse_date(&self, field: &str) -> GenericResult<Date> {
        parse_date(self.get_value(field)?)
    }

    pub fn parse_amount(&self, field: &str, restrictions: DecimalRestrictions) -> GenericResult<Decimal> {
        let value = self.get_value(field)?;
        Ok(util::parse_decimal(&value.replace(',', ""), restrictions).map_err(|_| format!(
            "Invalid amount: {:?}", value))?)
    }

    pub fn parse_cash(&self, field: &str, currency: &str, restrictions: DecimalRestrictions) -> GenericResult<Cash> {
        Ok(Cash::new(currency, self.parse_amount(field, restrictions)?))
    }
}

pub trait RecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> { Some(&["Data"]) }
    fn skip_data_types(&self) -> Option<&'static [&'static str]> { None }
    fn skip_totals(&self) -> bool { false }
    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult;
}

pub struct UnknownRecordParser {}

impl RecordParser for UnknownRecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> {
        None
    }

    fn parse(&mut self, _parser: &mut StatementParser, _record: &Record) -> EmptyResult {
        Ok(())
    }
}

pub fn format_record<'a, I>(iter: I) -> String
    where I: IntoIterator<Item = &'a str> {

    iter.into_iter()
        .map(|value| format!("{:?}", value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%Y-%m-%d")
}

pub fn parse_date_time(date_time: &str) -> GenericResult<DateTime> {
    util::parse_date_time(date_time, "%Y-%m-%d, %H:%M:%S")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("2018-06-22").unwrap(), date!(22, 6, 2018));
    }

    #[test]
    fn time_parsing() {
        assert_eq!(parse_date_time("2018-07-31, 13:09:47").unwrap(), date!(31, 7, 2018).and_hms(13, 9, 47));
    }
}