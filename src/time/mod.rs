use std::ops::Add;

use chrono::{self, Duration, Local, TimeZone};
use chrono_tz::Tz;
#[cfg(debug_assertions)] use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::GenericResult;
use crate::formatting;

pub use crate::types::{Date, Time, DateTime};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Period {
    first: Date,
    last: Date,
}

impl Period {
    pub fn new(first: Date, last: Date) -> GenericResult<Period> {
        let period = Period {first, last};

        if period.first > period.last {
            return Err!("Invalid period: {}", period.format());
        }

        Ok(period)
    }

    pub fn prev_date(&self) -> Date {
        self.first.pred()
    }

    pub fn first_date(&self) -> Date {
        self.first
    }

    pub fn last_date(&self) -> Date {
        self.last
    }

    pub fn next_date(&self) -> Date {
        self.last.succ()
    }

    pub fn contains(&self, date: Date) -> bool {
        self.first <= date && date <= self.last
    }

    pub fn days(&self) -> i64 {
        (self.last - self.first).num_days() + 1
    }

    pub fn format(&self) -> String {
        format!("{} - {}", formatting::format_date(self.first), formatting::format_date(self.last))
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct DateOptTime {
    pub date: Date,
    pub time: Option<Time>,
}

impl DateOptTime {
    pub fn new_max_time(date: Date) -> DateOptTime {
        DateOptTime {date, time: Some(Time::from_hms_nano(23, 59, 59, 999_999_999))}
    }

    pub fn or_min_time(&self) -> DateTime {
        DateTime::new(self.date, match self.time {
            Some(time) => time,
            None => Time::from_hms(0, 0, 0),
        })
    }
}

impl From<Date> for DateOptTime {
    fn from(date: Date) -> Self {
        DateOptTime {date, time: None}
    }
}

impl From<DateTime> for DateOptTime {
    fn from(time: DateTime) -> Self {
        DateOptTime {
            date: time.date(),
            time: Some(time.time()),
        }
    }
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

pub fn deserialize_date_opt_time<'de, D>(deserializer: D) -> Result<DateOptTime, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    parse_date_time(&value, "%Y.%m.%d %H:%M:%S").map(Into::into)
        .or_else(|_| parse_date(&value, "%d.%m.%Y").map(Into::into))
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

pub fn today() -> Date {
    tz_now().date().naive_local()
}

pub fn today_trade_conclusion_time() -> DateOptTime {
    now().into()
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