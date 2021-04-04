pub use chrono::NaiveDate as Date;
pub use chrono::NaiveTime as Time;
pub use chrono::NaiveDateTime as DateTime;

pub use rust_decimal::Decimal as Decimal;
pub const DECIMAL_PRECISION: u32 = 28;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TradeType {
    Buy,
    Sell,
}

macro_rules! date {
    ($day:expr, $month:expr, $year:expr) => (::chrono::NaiveDate::from_ymd($year, $month, $day))
}

#[cfg(test)]
macro_rules! date_time {
    ($hour:expr, $minute:expr, $second:expr, $day:expr, $month:expr, $year:expr) => {
        ::chrono::NaiveDateTime::new(
            date!($day, $month, $year),
            ::chrono::NaiveTime::from_hms($hour, $minute, $second),
        )
    }
}