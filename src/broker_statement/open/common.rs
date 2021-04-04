use std::collections::HashMap;

use num_traits::{FromPrimitive, ToPrimitive};
use serde::{de::Error, Deserialize, Deserializer};

use crate::core::GenericResult;
use crate::time::{self, Date, Time, DateTime};
use crate::types::Decimal;

fn parse_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%Y-%m-%dT00:00:00")
}

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<Date, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    parse_date(&value).map_err(D::Error::custom)
}

fn parse_time(time: &str) -> GenericResult<Time> {
    time::parse_time(time, "1900-01-01T%H:%M:%S")
}

pub fn deserialize_time<'de, D>(deserializer: D) -> Result<Time, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    parse_time(&value).map_err(D::Error::custom)
}

fn parse_date_time(time: &str) -> GenericResult<DateTime> {
    time::parse_date_time(time, "%Y-%m-%dT%H:%M:%S")
}

pub fn deserialize_date_time<'de, D>(deserializer: D) -> Result<DateTime, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    parse_date_time(&value).map_err(D::Error::custom)
}

pub fn parse_quantity(decimal_quantity: Decimal, allow_zero: bool) -> GenericResult<u32> {
    Ok(decimal_quantity.to_u32().and_then(|quantity| {
        if Decimal::from_u32(quantity).unwrap() != decimal_quantity {
            return None;
        }

        if !allow_zero && quantity == 0 {
            return None;
        }

        Some(quantity)
    }).ok_or_else(|| format!("Invalid quantity: {}", decimal_quantity))?)
}

pub fn get_symbol<'a>(securities: &'a HashMap<String, String>, name: &str) -> GenericResult<&'a str> {
    Ok(securities.get(name).ok_or_else(|| format!(
        "Unable to find security info by its name ({:?})", name))?.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("2017-12-31T00:00:00").unwrap(), date!(31, 12, 2017));
    }

    #[test]
    fn time_parsing() {
        assert_eq!(parse_time("1900-01-01T12:20:25").unwrap(), Time::from_hms(12, 20, 25));
    }

    #[test]
    fn date_time_parsing() {
        assert_eq!(
            parse_date_time("2021-02-20T12:31:44").unwrap(),
            date_time!(12, 31, 44, 20, 2, 2021),
        );
    }
}