use std::borrow::Cow;

use lazy_static::lazy_static;
use regex::Regex;

use crate::core::GenericResult;
use crate::formats::xls::{self, Cell};
use crate::time::{self, Date, Time};

pub fn parse_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%d.%m.%Y")
}

pub fn parse_short_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%d.%m.%y")
}

pub fn parse_short_date_cell(cell: &Cell) -> GenericResult<Date> {
    parse_short_date(xls::get_string_cell(cell)?)
}

fn parse_time(time: &str) -> GenericResult<Time> {
    time::parse_time(time, "%H:%M:%S")
}

pub fn parse_time_cell(cell: &Cell) -> GenericResult<Time> {
    parse_time(xls::get_string_cell(cell)?)
}

pub fn map_currency(name: &str) -> Option<&'static str> {
    Some(match name {
        "Рубль" => "RUB",
        _ => return None,
    })
}

pub fn parse_currency(name: &str) -> GenericResult<&'static str> {
    Ok(map_currency(name).ok_or_else(|| format!("Unsupported currency: {name:?}"))?)
}

pub fn parse_symbol(name: &str) -> GenericResult<String> {
    lazy_static! {
        static ref SYMBOL_REGEX: Regex = Regex::new(
            r"^(?P<symbol>[A-Z][A-Z0-9]*)(?:[._][A-Z]+)?$").unwrap();
    }

    let captures = SYMBOL_REGEX.captures(name).ok_or_else(|| format!(
        "Invalid instrument symbol: {name:?}"))?;

    Ok(captures.name("symbol").unwrap().as_str().to_owned())
}

pub fn trim_column_title(title: &str) -> Cow<'_, str> {
    lazy_static! {
        static ref FOOTNOTE_REFERENCE_REGEX: Regex = Regex::new(r"\s*\(\d+\*\)\s*$").unwrap();
    }
    FOOTNOTE_REFERENCE_REGEX.replace(title, "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest(name, symbol,
        case("FXDM_RM", "FXDM"),
        case("FXRL_RX", "FXRL"),
        case("FXRU.MRG", "FXRU"),
    )]
    fn symbol_parsing(name: &str, symbol: &str) {
        assert_eq!(parse_symbol(name).unwrap(), symbol);
    }

    #[rstest(input, expected,
        case("Тип сделки", "Тип сделки"),
        case("Тип сделки (20*)", "Тип сделки"),
    )]
    fn column_title_trimming(input: &str, expected: &str) {
        assert_eq!(trim_column_title(input), expected);
    }
}