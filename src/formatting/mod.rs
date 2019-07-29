use chrono::Duration;

use crate::types::Date;

pub mod static_table;
pub mod table;

pub fn format_date(date: Date) -> String {
    date.format("%d.%m.%Y").to_string()
}

pub fn format_period(start: Date, end: Date) -> String {
    format!("{} - {}", format_date(start), format_date(end - Duration::days(1)))
}