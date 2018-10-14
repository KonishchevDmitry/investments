pub use rust_decimal::Decimal as Decimal;
pub use chrono::NaiveDate as Date;

macro_rules! decs {
    ($value:expr) => {{
        use ::std::str::FromStr;
        ::rust_decimal::Decimal::from_str($value).unwrap()
    }}
}

macro_rules! dec {
    ($value:expr) => (::rust_decimal::Decimal::from($value))
}

#[cfg(test)]
macro_rules! date {
    ($day:expr, $month:expr, $year:expr) => (::chrono::NaiveDate::from_ymd($year, $month, $day))
}