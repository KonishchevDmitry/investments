mod date;
mod parsing;
mod period;

use std::ops::Add;

use chrono::{self, Duration, Local};
#[cfg(debug_assertions)] use lazy_static::lazy_static;

pub use crate::types::{Date, Time, DateTime};

pub use date::*;
pub use parsing::*;
pub use period::*;

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
            static ref FAKE_NOW: Option<chrono::DateTime<Local>> = parsing::parse_fake_now().unwrap_or_else(|e| {
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