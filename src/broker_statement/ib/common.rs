use std::iter::Iterator;
use std::str::FromStr;

use chrono::NaiveDateTime;
use csv::StringRecord;
use num_traits::Zero;

use broker_statement::ib::IbStatementParser;
use core::{EmptyResult, GenericResult};
use types::{Date, Decimal};
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

    pub fn parse_value<T: FromStr>(&self, field: &str) -> GenericResult<T> {
        let value = self.get_value(field)?;
        Ok(value.parse().map_err(|_| format!(
            "{:?} field has an invalid value: {:?}", field, value))?)
    }

    pub fn parse_cash(&self, field: &str, cash_type: CashType) -> GenericResult<Decimal> {
        let value = self.get_value(field)?;
        let amount = Decimal::from_str(&value.replace(',', "")).map_err(|_| format!(
            "Invalid amount: {:?}", value))?;

        if !match cash_type {
            CashType::NonZero => !amount.is_zero(),
            CashType::NegativeOrZero => amount.is_sign_negative() || amount.is_zero(),
            CashType::StrictlyPositive => amount.is_sign_positive() && !amount.is_zero(),
        } {
            return Err!("Invalid amount: {:?}", value);
        }

        Ok(amount)
    }
}

pub trait RecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> { Some(&["Data"]) }
    fn skip_data_types(&self) -> Option<&'static [&'static str]> { None }
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult;
}

pub enum CashType {
    NonZero,
    NegativeOrZero,
    StrictlyPositive,
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

pub fn parse_time(time: &str) -> GenericResult<NaiveDateTime> {
    Ok(NaiveDateTime::parse_from_str(time, "%Y-%m-%d, %H:%M:%S").map_err(|_| format!(
        "Invalid time: {:?}", time))?)
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
        assert_eq!(parse_time("2018-07-31, 13:09:47").unwrap(), date!(31, 7, 2018).and_hms(13, 9, 47));
    }
}