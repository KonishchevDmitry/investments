use lazy_static::lazy_static;
use regex::Regex;

use crate::core::GenericResult;
use crate::time::{self, Date, Time};

pub fn parse_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%d.%m.%Y")
}

pub fn parse_short_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%d.%m.%y")
}

pub fn parse_time(time: &str) -> GenericResult<Time> {
    time::parse_time(time, "%H:%M:%S")
}

pub fn map_currency(name: &str) -> Option<&'static str> {
    Some(match name {
        "Рубль" => "RUB",
        _ => return None,
    })
}

pub fn parse_currency(name: &str) -> GenericResult<&'static str> {
    Ok(map_currency(name).ok_or_else(|| format!("Unsupported currency: {:?}", name))?)
}

pub fn parse_symbol(name: &str) -> GenericResult<String> {
    // For now use hardcoded ISIN mapping here
    if name == "RU000A101X76" {
        return Ok(s!("TMOS"))
    }

    lazy_static! {
        static ref SYMBOL_REGEX: Regex = Regex::new(
            r"^(?P<symbol>[A-Z]+)(?:[._][A-Z]+)?$").unwrap();
    }

    let captures = SYMBOL_REGEX.captures(name).ok_or_else(|| format!(
        "Invalid instrument symbol: {:?}", name))?;

    Ok(captures.name("symbol").unwrap().as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest(name, symbol,
        case("FXRL_RX", "FXRL"),
        case("FXRU.MRG", "FXRU"),
        case("RU000A101X76", "TMOS"),
    )]
    fn symbol_parsing(name: &str, symbol: &str) {
        assert_eq!(parse_symbol(name).unwrap(), symbol);
    }
}