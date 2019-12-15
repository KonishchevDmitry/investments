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