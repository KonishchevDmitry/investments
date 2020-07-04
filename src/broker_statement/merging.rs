use chrono::{Datelike, Weekday};

use crate::core::EmptyResult;
use crate::formatting;
use crate::types::Date;

#[derive(Debug, Clone, Copy)]
pub enum StatementsMergingStrategy {
    ContinuousOnly,
    SparseOnHolidays(usize),
    Sparse,
}

impl StatementsMergingStrategy {
    pub fn validate(self, first: (Date, Date), second: (Date, Date)) -> EmptyResult {
        let error = |message| {
            let first = formatting::format_period(first);
            let second = formatting::format_period(second);
            Err!("{}: {}, {}", message, first, second)
        };

        if second.0 < first.1 {
            return error("Overlapping periods");
        }

        match self {
            StatementsMergingStrategy::ContinuousOnly => {
                if second.0 != first.1 {
                    return error("Non-continuous periods");
                }
            },
            StatementsMergingStrategy::SparseOnHolidays(max_days) => {
                let mut date = first.1;
                let mut missing_days = 0;

                while date < second.0 {
                    if !matches!(date.weekday(), Weekday::Sat | Weekday::Sun) {
                        if missing_days >= max_days {
                            return error("Non-continuous periods");
                        }
                        missing_days += 1;
                    }
                    date = date.succ();
                }
            }
            StatementsMergingStrategy::Sparse => {},
        };

        Ok(())
    }
}