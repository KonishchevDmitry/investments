use chrono::Datelike;

use super::{Date, Period};

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub struct Month {
    year: i32,
    month: u32,
}

impl From<Date> for Month {
    fn from(date: Date) -> Self {
        Month {
            year: date.year(),
            month: date.month(),
        }
    }
}

impl Month {
    pub fn day(&self, day: u32) -> Option<Date> {
        Date::from_ymd_opt(self.year, self.month, day)
    }

    pub fn day_or_last(&self, day: u32) -> Date {
        self.day(day).unwrap_or_else(|| self.period().last_date())
    }

    pub fn period(&self) -> Period {
        let first_day = date!(self.year, self.month, 1);
        let next_month = self.next();
        let last_day = date!(next_month.year, next_month.month, 1).pred();
        Period::new(first_day, last_day).unwrap()
    }

    pub fn prev(mut self) -> Month {
        if self.month == 1 {
            self.year -= 1;
            self.month = 12;
        } else {
            self.month -= 1;
        }
        self
    }

    pub fn next(mut self) -> Month {
        if self.month == 12 {
            self.year += 1;
            self.month = 1;
        } else {
            self.month += 1;
        }
        self
    }
}