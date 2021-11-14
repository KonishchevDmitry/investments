use chrono::{Datelike, Weekday};

use crate::core::EmptyResult;
use crate::time::{Date, Period};

#[derive(Clone, Copy)]
pub enum StatementsMergingStrategy {
    ContinuousOnly,
    SparseOnHolidays(usize),
    SparseSingleDaysLastMonth,
}

impl StatementsMergingStrategy {
    pub fn validate(self, first: Period, second: Period, last_date: Date) -> EmptyResult {
        let error = |message| Err!("{}: {}, {}", message, first.format(), second.format());

        if second.first_date() <= first.last_date() {
            return error("Overlapping periods");
        }

        match self {
            StatementsMergingStrategy::ContinuousOnly => {
                if second.first_date() != first.next_date() {
                    return error("Non-continuous periods");
                }
            },

            StatementsMergingStrategy::SparseSingleDaysLastMonth => {
                if second.first_date() != first.next_date() {
                    let last_month = (last_date.year(), last_date.month());

                    if second.days() == 1 && (second.first_date().year(), second.first_date().month()) == last_month {
                        // Some brokers allow to generate only daily statements for the current month
                    } else {
                        return error("Non-continuous periods");
                    }
                }
            },

            StatementsMergingStrategy::SparseOnHolidays(max_days) => {
                let mut date = first.next_date();
                let mut missing_days = 0;

                while date < second.first_date() {
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