use serde::{de::Error, Deserialize, Deserializer};

use crate::core::GenericResult;
use crate::time::{self, Time};

fn parse_time(time: &str) -> GenericResult<Time> {
    time::parse_time(time, "%H:%M")
}

pub fn deserialize_time<'de, D>(deserializer: D) -> Result<Time, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    parse_time(&value).map_err(D::Error::custom)
}

pub fn parse_security_code(code: &str) -> GenericResult<&str> {
    match code.strip_suffix("_SPB") {
        Some(symbol) => Ok(symbol),
        None => Err!("Got a security code in an unexpected format: {:?}", code),
    }
}

#[cfg(test)]
mod tests {
    use matches::assert_matches;
    use super::*;

    #[test]
    fn time_parsing() {
        assert_eq!(parse_time("16:45").unwrap(), Time::from_hms(16, 45, 0));
    }

    #[test]
    fn security_code_parsing() {
        assert_matches!(parse_security_code("KO_SPB"), Ok("KO"));
        assert_matches!(parse_security_code("KO"), Err(_));
    }
}