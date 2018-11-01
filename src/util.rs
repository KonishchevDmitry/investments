use std::str::FromStr;

use num_traits::Zero;
use rust_decimal::RoundingStrategy;

use core::GenericResult;
use types::{Date, Decimal};

pub enum DecimalRestrictions {
    NonZero,
    NegativeOrZero,
    StrictlyPositive,
}

pub fn parse_decimal(string: &str, restrictions: DecimalRestrictions) -> GenericResult<Decimal> {
    let value = Decimal::from_str(string).map_err(|_| "Invalid decimal value")?;

    if !match restrictions {
        DecimalRestrictions::NonZero => !value.is_zero(),
        DecimalRestrictions::NegativeOrZero => value.is_sign_negative() || value.is_zero(),
        DecimalRestrictions::StrictlyPositive => value.is_sign_positive() && !value.is_zero(),
    } {
        return Err!("The value doesn't comply to the specified restrictions");
    }

    Ok(value)
}

pub fn round_to(value: Decimal, points: u32) -> Decimal {
    value.round_dp_with_strategy(points, RoundingStrategy::RoundHalfUp).normalize()
}

pub fn parse_date(date: &str, format: &str) -> GenericResult<Date> {
    Ok(Date::parse_from_str(date, format).map_err(|_| format!(
        "Invalid date: {:?}", date))?)
}

pub fn format_date(date: Date) -> String {
    date.format("%d.%m.%Y").to_string()
}