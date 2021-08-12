use chrono::{Datelike, Duration, Weekday};

use crate::core::EmptyResult;
use crate::formatting;
use crate::types::Date;

#[derive(Clone, Copy)]
pub enum StatementsMergingStrategy {
    ContinuousOnly,
    SparseOnHolidays(usize),
    SparseSingleDaysLastMonth,
}

impl StatementsMergingStrategy {
    pub fn validate(self, first: (Date, Date), second: (Date, Date), last_end_date: Date) -> EmptyResult {
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

            StatementsMergingStrategy::SparseSingleDaysLastMonth => {
                if second.0 != first.1 {
                    let last_month = {
                        let last_day = last_end_date.pred();
                        (last_day.year(), last_day.month())
                    };

                    if second.1 - second.0 == Duration::days(1) && (second.0.year(), second.0.month()) == last_month {
                        // Some brokers allow to generate only daily statements for the current month
                    } else {
                        return error("Non-continuous periods");
                    }
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
        };

        Ok(())
    }
}