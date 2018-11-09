use serde::{Deserialize, Deserializer};
use serde::de::Error;

use core::GenericResult;
use types::Date;
use util;

fn parse_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%Y-%m-%dT00:00:00")
}

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<Date, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    parse_date(&value).map_err(D::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("2017-12-31T00:00:00").unwrap(), date!(31, 12, 2017));
    }
}