use std::collections::HashMap;

use num_traits::{FromPrimitive, ToPrimitive};
use serde::{de::Error, Deserialize, Deserializer};

use crate::core::GenericResult;
use crate::time;
use crate::types::{Date, Decimal};

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<Date, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    parse_date(&value).map_err(D::Error::custom)
}

fn parse_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%Y-%m-%dT00:00:00")
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
}