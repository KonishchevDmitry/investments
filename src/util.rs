use rust_decimal::RoundingStrategy;

use core::GenericResult;
use types::{Date, Decimal};

pub fn parse_date(date: &str, format: &str) -> GenericResult<Date> {
    Ok(Date::parse_from_str(date, format).map_err(|_| format!(
        "Invalid date: {:?}", date))?)
}

pub fn format_date(date: Date) -> String {
    date.format("%d.%m.%Y").to_string()
}

pub fn round_to(value: Decimal, points: u32) -> Decimal {
    value.round_dp_with_strategy(points, RoundingStrategy::RoundHalfUp).normalize()
}