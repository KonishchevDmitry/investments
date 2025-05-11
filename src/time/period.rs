use std::fmt;

use crate::core::GenericResult;
use crate::formatting;

use super::Date;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
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

    pub fn contains_period(&self, other: Period) -> bool {
        self.first <= other.first && self.last >= other.last
    }

    pub fn try_intersect(&self, other: Period) -> Option<Period> {
        let (earlier, later) = if self.first <= other.first {
            (*self, other)
        } else {
            (other, *self)
        };

        if later.first > earlier.last {
            return None;
        }

        Some(Period {
            first: later.first,
            last: std::cmp::min(earlier.last, later.last),
        })
    }

    pub fn try_union(&self, other: Period) -> Option<Period> {
        let (earlier, later) = if self.first <= other.first {
            (*self, other)
        } else {
            (other, *self)
        };

        if later.first > earlier.next_date() {
            return None;
        }

        Some(Period {
            first: earlier.first,
            last: std::cmp::max(earlier.last, later.last),
        })
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

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(first, second, expected,
        case(
            Period::new(date!(2000, 1, 1), date!(2000, 2, 1)),
            Period::new(date!(2000, 2, 2), date!(2000, 3, 1)),
            Some(Period::new(date!(2000, 1, 1), date!(2000, 3, 1))),
        ),
        case(
            Period::new(date!(2000, 1, 1), date!(2000, 2, 1)),
            Period::new(date!(2000, 2, 1), date!(2000, 3, 1)),
            Some(Period::new(date!(2000, 1, 1), date!(2000, 3, 1))),
        ),
        case(
            Period::new(date!(2000, 1, 1), date!(2000, 2, 1)),
            Period::new(date!(2000, 1, 31), date!(2000, 3, 1)),
            Some(Period::new(date!(2000, 1, 1), date!(2000, 3, 1))),
        ),
        case(
            Period::new(date!(2000, 1, 1), date!(2000, 2, 1)),
            Period::new(date!(2000, 1, 1), date!(2000, 2, 1)),
            Some(Period::new(date!(2000, 1, 1), date!(2000, 2, 1))),
        ),
        case(
            Period::new(date!(2000, 1, 1), date!(2000, 2, 1)),
            Period::new(date!(2000, 2, 3), date!(2000, 3, 1)),
            None,
        ),
    )]
    fn union(first: GenericResult<Period>, second: GenericResult<Period>, expected: Option<GenericResult<Period>>) {
        let first = first.unwrap();
        let second = second.unwrap();
        let expected = expected.transpose().unwrap();

        assert_eq!(first.try_union(second), expected);
        assert_eq!(second.try_union(first), expected);
    }
}