pub use chrono::NaiveDate as Date;
pub use chrono::NaiveDateTime as DateTime;
pub use rust_decimal::Decimal as Decimal;

// TODO: Waiting for https://github.com/paupino/rust-decimal/issues/151
macro_rules! dec {
    ($value:expr) => (::rust_decimal::Decimal::from($value))
}
macro_rules! decf {
    ($value:expr) => {{
        use ::std::str::FromStr;
        ::rust_decimal::Decimal::from_str(stringify!($value)).unwrap()
    }}
}

macro_rules! date {
    ($day:expr, $month:expr, $year:expr) => (::chrono::NaiveDate::from_ymd($year, $month, $day))
}