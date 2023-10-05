use chrono::{self, Duration, TimeZone};
#[cfg(debug_assertions)] use chrono::Local;
use chrono_tz::Tz;
use regex::Regex;
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::GenericResult;

use super::{Date, Time, DateTime, DateOptTime, TzDateTime, utc_now};

pub fn parse_date(date: &str, format: &str) -> GenericResult<Date> {
    Ok(Date::parse_from_str(date, format).map_err(|_| format!(
        "Invalid date: {:?}", date))?)
}

pub fn parse_user_date(date: &str) -> GenericResult<Date> {
    parse_date(date, "%Y.%m.%d").or_else(|_| parse_date(date, "%d.%m.%Y"))
}

pub fn parse_time(time: &str, format: &str) -> GenericResult<Time> {
    Ok(Time::parse_from_str(time, format).map_err(|_| format!(
        "Invalid time: {:?}", time))?)
}

pub fn parse_date_time(date_time: &str, format: &str) -> GenericResult<DateTime> {
    Ok(DateTime::parse_from_str(date_time, format).map_err(|_| format!(
        "Invalid time: {:?}", date_time))?)
}

pub fn parse_tz_date_time<T: TimeZone>(
    string: &str, format: &str, tz: T, future_check: bool,
) -> GenericResult<TzDateTime<T>> {
    let date_time = chrono::NaiveDateTime::parse_from_str(string, format).ok()
        .and_then(|date_time| tz.from_local_datetime(&date_time).single())
        .ok_or_else(|| format!("Invalid time: {:?}", string))?;

    if future_check && (date_time.naive_utc() - utc_now()).num_hours() > 0 {
        return Err!("Invalid time: {:?}. It's from future", date_time);
    }

    Ok(date_time)
}

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<Date, D::Error>
    where D: Deserializer<'de>
{
    let date: String = Deserialize::deserialize(deserializer)?;
    parse_user_date(&date).map_err(D::Error::custom)
}

pub fn deserialize_date_opt_time<'de, D>(deserializer: D) -> Result<DateOptTime, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    parse_date_time(&value, "%Y.%m.%d %H:%M:%S").map(Into::into)
        .or_else(|_| parse_user_date(&value).map(Into::into))
        .map_err(D::Error::custom)
}

pub fn parse_timezone(timezone: &str) -> GenericResult<Tz> {
    Ok(timezone.parse().map_err(|_| format!("Invalid time zone: {:?}", timezone))?)
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

#[cfg(debug_assertions)]
pub fn parse_fake_now() -> GenericResult<Option<TzDateTime<Local>>> {
    use std::env::{self, VarError};

    let name = "INVESTMENTS_NOW";

    let fake_now = match env::var(name) {
        Ok(value) => {
            chrono::NaiveDateTime::parse_from_str(&value, "%Y.%m.%d %H:%M:%S").ok()
                .and_then(|date_time| Local.from_local_datetime(&date_time).single())
        },
        Err(e) => match e {
            VarError::NotPresent => return Ok(None),
            VarError::NotUnicode(_) => None,
        },
    }.ok_or_else(|| format!("Invalid {} environment variable value", name))?;

    Ok(Some(fake_now))
}