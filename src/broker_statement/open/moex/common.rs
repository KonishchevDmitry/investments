use std::collections::HashMap;

use serde::{de::Error, Deserialize, Deserializer};

use crate::core::GenericResult;
use crate::time::{self, Time, DateTime};

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

pub fn get_symbol<'a>(securities: &'a HashMap<String, String>, name: &str) -> GenericResult<&'a str> {
    Ok(securities.get(name).ok_or_else(|| format!(
        "Unable to find security info by its name ({:?})", name))?.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_parsing() {
        assert_eq!(
            parse_time("1900-01-01T12:20:25").unwrap(),
            Time::from_hms(12, 20, 25),
        );
    }

    #[test]
    fn date_time_parsing() {
        assert_eq!(
            parse_date_time("2021-02-20T12:31:44").unwrap(),
            date_time!(2021, 2, 20, 12, 31, 44),
        );
    }
}