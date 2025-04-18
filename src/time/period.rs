use std::fmt;

use crate::core::GenericResult;
use crate::formatting;

use super::Date;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Period {
    first: Date,
    last: Date,
}

impl Period {
    pub fn new(first: Date, last: Date) -> GenericResult<Period> {
        let period = Period {first, last};

        if period.first > period.last {
            return Err!("Invalid period: {period}");
        }

        Ok(period)
    }

    pub fn join(mut self, other: Period) -> GenericResult<Period> {
        if other.first_date() != self.next_date() {
            return Err!("Non-continuous periods: {self}, {other}");
        }

        self.last = other.last;
        Ok(self)
    }

    pub fn prev_date(&self) -> Date {
        self.first.pred_opt().unwrap()
    }

    pub fn first_date(&self) -> Date {
        self.first
    }

    pub fn last_date(&self) -> Date {
        self.last
    }

    pub fn next_date(&self) -> Date {
        self.last.succ_opt().unwrap()
    }

    pub fn contains(&self, date: Date) -> bool {
        self.first <= date && date <= self.last
    }

    pub fn days(&self) -> i64 {
        (self.last - self.first).num_days() + 1
    }
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {}", formatting::format_date(self.first), formatting::format_date(self.last))
    }
}