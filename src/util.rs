use std::ops::Neg;
use std::str::FromStr;

use chrono::{self, Duration, Local};
use num_traits::Zero;
use regex::Regex;
use rust_decimal::RoundingStrategy;

use crate::core::GenericResult;
use crate::types::{Date, DateTime, Decimal};

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
    let mut round_value = value.round_dp_with_strategy(points, RoundingStrategy::RoundHalfUp);

    if round_value.is_zero() && round_value.is_sign_negative() {
        round_value = round_value.neg();
    }

    round_value.normalize()
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
    }).ok_or_else(|| format!("Invalid duration: {}", string))?;

    Ok(Duration::seconds(seconds))
}

pub fn today() -> Date {
    tz_now().date().naive_local()
}

pub fn now() -> DateTime {
    tz_now().naive_local()
}

pub fn utc_now() -> DateTime {
    tz_now().naive_utc()
}

fn tz_now() -> chrono::DateTime<Local> {
    #[cfg(debug_assertions)]
    {
        use std::process;
        use lazy_static::lazy_static;

        lazy_static! {
            static ref FAKE_NOW: Option<chrono::DateTime<Local>> = parse_fake_now().unwrap_or_else(|e| {
                eprintln!("{}.", e);
                process::exit(1);
            });
        }

        if let Some(&now) = FAKE_NOW.as_ref() {
            return now;
        }
    }

    chrono::Local::now()
}

#[cfg(debug_assertions)]
fn parse_fake_now() -> GenericResult<Option<chrono::DateTime<Local>>> {
    use std::env::{self, VarError};
    use chrono::offset::TimeZone;

    let name = "INVESTMENTS_NOW";

    match env::var(name) {
        Ok(value) => {
            let timezone = chrono::Local::now().timezone();
            if let Ok(now) = timezone.datetime_from_str(&value, "%Y.%m.%d %H:%M:%S") {
                return Ok(Some(now));
            }
        },
        Err(e) => match e {
            VarError::NotPresent => return Ok(None),
            VarError::NotUnicode(_) => {},
        },
    };

    Err!("Invalid {} environment variable value", name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding() {
        assert_eq!(round_to(dec!(-1.5), 0), dec!(-2));
        assert_eq!(round_to(dec!(-1.4), 0), dec!(-1));
        assert_eq!(round_to(dec!(-0.5), 0), dec!(-1));
        assert_eq!(round_to(dec!(-0.4), 0), dec!(0));
        assert_eq!(round_to(dec!(0.4), 0), dec!(0));
        assert_eq!(round_to(dec!(0.5), 0), dec!(1));
        assert_eq!(round_to(dec!(1.4), 0), dec!(1));
        assert_eq!(round_to(dec!(1.5), 0), dec!(2));
    }
}