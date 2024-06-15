mod date;
mod month;
mod parsing;
mod period;

use chrono::{Local, TimeZone};
#[cfg(debug_assertions)] use lazy_static::lazy_static;

pub use chrono::DateTime as TzDateTime;
pub use crate::types::{Date, Time, DateTime};

pub use date::*;
pub use month::*;
pub use parsing::*;
pub use period::*;

pub fn today() -> Date {
    tz_now().naive_local().date()
}

pub fn now() -> DateTime {
    tz_now().naive_local()
}

pub fn utc_now() -> DateTime {
    tz_now().naive_utc()
}

pub fn timestamp() -> i64 {
    utc_now().and_utc().timestamp()
}

fn tz_now() -> TzDateTime<Local> {
    #[cfg(debug_assertions)]
    {
        use std::process;

        lazy_static! {
            static ref FAKE_NOW: Option<TzDateTime<Local>> = parsing::parse_fake_now().unwrap_or_else(|e| {
                eprintln!("{}.", e);
                process::exit(1);
            });
        }

        if let Some(&now) = FAKE_NOW.as_ref() {
            return now;
        }
    }

    Local::now()
}

pub trait TimeProvider: Sync + Send {
    fn now(&self) -> TzDateTime<Local>;
}

pub struct SystemTime();

impl TimeProvider for SystemTime {
    fn now(&self) -> TzDateTime<Local> {
        tz_now()
    }
}

pub struct FakeTime(i64);

impl FakeTime {
    pub fn new<T: TimeZone>(time: TzDateTime<T>) -> FakeTime {
        FakeTime(time.timestamp())
    }
}

impl TimeProvider for FakeTime {
    fn now(&self) -> TzDateTime<Local> {
        Local.from_utc_datetime(&chrono::DateTime::from_timestamp(self.0, 0).unwrap().naive_utc())
    }
}