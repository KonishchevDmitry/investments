use crate::time::{DateTime, DateOptTime};
use crate::types::Decimal;
use crate::util;

pub mod table;

pub fn format_date<T>(date: T) -> String where T: Into<DateOptTime> {
    let date = date.into();

    if let Some(time) = date.time {
        DateTime::new(date.date, time).format("%H:%M:%S %d.%m.%Y")
    } else {
        date.date.format("%d.%m.%Y")
    }.to_string()
}

pub fn format_days(days: u32) -> String {
    let (duration_name, duration_days) = if days >= 365 {
        ("y", 365)
    } else if days >= 30 {
        ("m", 30)
    } else {
        ("d", 1)
    };
    format!("{}{duration_name}", util::round(Decimal::from(days) / Decimal::from(duration_days), 1))
}

pub fn untitle(string: &str) -> String {
    let mut result = String::with_capacity(string.len());

    let mut chars = string.chars();
    if let Some(char) = chars.next() {
        result.extend(char.to_lowercase());
        result.extend(chars);
    }

    result
}