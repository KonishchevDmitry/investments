use std::ops::Add;

use chrono::{self, Duration, Local, TimeZone};
use chrono_tz::Tz;
#[cfg(debug_assertions)] use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::GenericResult;
use crate::formatting;
use crate::types::{Date, Time, DateTime};

pub fn parse_period(start: Date, end: Date) -> GenericResult<(Date, Date)> {
    let period = (start, end.succ());

    if period.0 >= period.1 {
        return Err!("Invalid period: {}", formatting::format_period(period));
    }

    Ok(period)
}

pub fn parse_date(date: &str, format: &str) -> GenericResult<Date> {
    Ok(Date::parse_from_str(date, format).map_err(|_| format!(
        "Invalid date: {:?}", date))?)
}

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<Date, D::Error>
    where D: Deserializer<'de>
{
    let date: String = Deserialize::deserialize(deserializer)?;
    parse_date(&date, "%d.%m.%Y").map_err(D::Error::custom)
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
) -> GenericResult<chrono::DateTime<T>> {
    let date_time = tz.datetime_from_str(string, format).map_err(|_| format!(
        "Invalid time: {:?}", string))?;

    if future_check && (date_time.naive_utc() - utc_now()).num_hours() > 0 {
        return Err!("Invalid time: {:?}. It's from future", date_time);
    }

    Ok(date_time)
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

pub fn today() -> Date {
    tz_now().date().naive_local()
}

pub fn today_trade_conclusion_date() -> Date {
    today()
}

pub fn today_trade_execution_date() -> Date {
    today().add(Duration::days(2))
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