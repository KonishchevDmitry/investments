use lazy_static::lazy_static;
use regex::Regex;

use crate::core::GenericResult;
use crate::types::Date;
use crate::util;

pub fn parse_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%d.%m.%Y")
}

pub fn parse_short_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%d.%m.%y")
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

    #[test]
    fn symbol_parsing() {
        assert_eq!(parse_symbol("FXRL_RX").unwrap(), s!("FXRL"));
        assert_eq!(parse_symbol("FXRU.MRG").unwrap(), s!("FXRU"));
    }
}