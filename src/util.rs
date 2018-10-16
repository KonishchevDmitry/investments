use core::GenericResult;
use types::Date;

pub fn parse_date(date: &str, format: &str) -> GenericResult<Date> {
    Ok(Date::parse_from_str(date, format).map_err(|_| format!(
        "Invalid date: {:?}", date))?)
}

pub fn format_date(date: Date) -> String {
    date.format("%d.%m.%Y").to_string()
}