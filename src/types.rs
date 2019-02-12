pub use chrono::NaiveDate as Date;
pub use chrono::NaiveDateTime as DateTime;
pub use rust_decimal::Decimal as Decimal;

macro_rules! date {
    ($day:expr, $month:expr, $year:expr) => (::chrono::NaiveDate::from_ymd($year, $month, $day))
}