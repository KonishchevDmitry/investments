use std::fmt::Write;

use chrono::Duration;

use core::{EmptyResult, GenericResult};
use types::{Date, Decimal};

pub trait TaxStatementType: Sized {
    fn decode(data: &str) -> GenericResult<Self>;
    fn encode(value: &Self, buffer: &mut String) -> EmptyResult;
}

impl TaxStatementType for usize {
    fn decode(data: &str) -> GenericResult<usize> {
        Ok(data.parse().map_err(|_| format!("Invalid usize value: {:?}", data))?)
    }

    fn encode(value: &usize, buffer: &mut String) -> EmptyResult {
        Ok(write!(buffer, "{}", value)?)
    }
}

impl TaxStatementType for bool {
    fn decode(data: &str) -> GenericResult<bool> {
        Ok(match data {
            "0" => false,
            "1" => true,
            _ => return Err!("Invalid bool value: {:?}", data),
        })
    }

    fn encode(value: &bool, buffer: &mut String) -> EmptyResult {
        Ok(buffer.push(match value {
            false => '0',
            true => '1',
        }))
    }
}

impl TaxStatementType for String {
    fn decode(data: &str) -> GenericResult<String> {
        Ok(data.to_owned())
    }

    fn encode(value: &String, buffer: &mut String) -> EmptyResult {
        Ok(buffer.push_str(&value))
    }
}

impl TaxStatementType for Date {
    fn decode(data: &str) -> GenericResult<Date> {
        let days = Duration::days(data.parse().map_err(|_| format!(
            "Invalid integer value: {:?}", data))?);

        Ok(get_base_date() + days)
    }

    fn encode(value: &Date, buffer: &mut String) -> EmptyResult {
        let days = (*value - get_base_date()).num_days();
        Ok(write!(buffer, "{}", days)?)
    }
}

fn get_base_date() -> Date {
    date!(30, 12, 1899)
}