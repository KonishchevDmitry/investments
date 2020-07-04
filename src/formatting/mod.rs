use crate::types::Date;

pub mod table;

pub fn format_date(date: Date) -> String {
    date.format("%d.%m.%Y").to_string()
}

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