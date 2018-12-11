use std::str::FromStr;

use chrono::{self, Duration};
use num_traits::Zero;
use regex::Regex;
use rust_decimal::RoundingStrategy;

use core::GenericResult;
use types::{Date, DateTime, Decimal};

pub enum DecimalRestrictions {
    NonZero,
    NegativeOrZero,
    PositiveOrZero,
    StrictlyPositive,
}

pub fn parse_decimal(string: &str, restrictions: DecimalRestrictions) -> GenericResult<Decimal> {
    let value = Decimal::from_str(string).map_err(|_| "Invalid decimal value")?;
    validate_decimal(value, restrictions)
}

pub fn validate_decimal(value: Decimal, restrictions: DecimalRestrictions) -> GenericResult<Decimal> {
    if !match restrictions {
        DecimalRestrictions::NonZero => !value.is_zero(),
        DecimalRestrictions::NegativeOrZero => value.is_sign_negative() || value.is_zero(),
        DecimalRestrictions::PositiveOrZero => value.is_sign_positive() || value.is_zero(),
        DecimalRestrictions::StrictlyPositive => value.is_sign_positive() && !value.is_zero(),
    } {
        return Err!("The value doesn't comply to the specified restrictions");
    }

    Ok(value)
}

pub fn round_to(value: Decimal, points: u32) -> Decimal {
    value.round_dp_with_strategy(points, RoundingStrategy::RoundHalfUp).normalize()
}

pub fn today() -> Date {
    chrono::Local::today().naive_local()
}

pub fn parse_date(date: &str, format: &str) -> GenericResult<Date> {
    Ok(Date::parse_from_str(date, format).map_err(|_| format!(
        "Invalid date: {:?}", date))?)
}

pub fn parse_date_time(date_time: &str, format: &str) -> GenericResult<DateTime> {
    Ok(DateTime::parse_from_str(date_time, format).map_err(|_| format!(
        "Invalid time: {:?}", date_time))?)
}

pub fn parse_duration(string: &str) -> GenericResult<Duration> {
    let re = Regex::new(r"^(?P<number>[1-9]\d*)(?P<unit>[mhd])$").unwrap();

    let seconds = re.captures(string).and_then(|captures| {
        let mut duration = match captures.name("number").unwrap().as_str().parse::<i64>().ok() {
            Some(duration) if duration > 0 => duration,
            _ => return None,
        };

        duration *= match captures.name("unit").unwrap().as_str() {
            "m" => 60,
            "h" => 60 * 60,
            "d" => 60 * 60 * 24,
            _ => unreachable!(),
        };

        Some(duration)
    }).ok_or(format!("Invalid duration: {}", string))?;

    Ok(Duration::seconds(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding() {
        assert_eq!(round_to(decf!(-1.5), 0), dec!(-2));
        assert_eq!(round_to(decf!(-1.4), 0), dec!(-1));
        assert_eq!(round_to(decf!(1.4), 0), dec!(1));
        assert_eq!(round_to(decf!(1.5), 0), dec!(2));
    }
}