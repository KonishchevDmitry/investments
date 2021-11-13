use crate::time::{Date, DateTime, DateOptTime};

pub mod table;

pub fn format_date<T>(date: T) -> String where T: Into<DateOptTime> {
    let date = date.into();

    if let Some(time) = date.time {
        DateTime::new(date.date, time).format("%H:%M:%S %d.%m.%Y")
    } else {
        date.date.format("%d.%m.%Y")
    }.to_string()
}

// FIXME(konishchev): Switch to Period?
pub fn format_period(period: (Date, Date)) -> String {
    format!("{} - {}", format_date(period.0), format_date(period.1.pred()))
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