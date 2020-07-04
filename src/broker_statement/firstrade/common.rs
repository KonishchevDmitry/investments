use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::{EmptyResult, GenericResult};
use crate::types::{Date, Decimal};
use crate::util;

#[derive(Deserialize)]
pub struct Ignore {
}

fn parse_date(date: &str) -> GenericResult<Date> {
    let format = match date.len() {
        14 => "%Y%m%d000000",
        _ => "%Y%m%d",
    };
    util::parse_date(date, format)
}

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<Date, D::Error> where D: Deserializer<'de> {
    let date: String = Deserialize::deserialize(deserializer)?;
    Ok(parse_date(&date).map_err(D::Error::custom)?)
}

pub fn deserialize_decimal<'de, D>(deserializer: D) -> Result<Decimal, D::Error> where D: Deserializer<'de> {
    #[derive(Deserialize)]
    pub struct Value {
        #[serde(rename = "$value")]
        pub value: Decimal,
    }

    let decimal: Value = Deserialize::deserialize(deserializer)?;
    Ok(decimal.value)
}

pub fn validate_sub_account(name: &str) -> EmptyResult {
    match name {
        "CASH" => Ok(()),
        _ => Err!("Got an unsupported sub-account type: {:?}", name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("20200623").unwrap(), date!(23, 6, 2020));
        assert_eq!(parse_date("20200623000000").unwrap(), date!(23, 6, 2020));
    }
}