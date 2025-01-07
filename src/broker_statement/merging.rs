use chrono::{Datelike, Weekday};

use crate::core::EmptyResult;
use crate::time::{Date, Month, Period};

#[derive(Clone, Copy)]
pub enum StatementsMergingStrategy {
    ContinuousOnly,

    Sparse,
    SparseOnHolidays(usize),

    // Some brokers allow to generate only daily statements for the current month. Monthly
    // statements become available later.
    SparseSingleDaysLastMonth(u32),
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
                Ok(())
            }

            StatementsMergingStrategy::Sparse => {
                Ok(())
            }

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
                    date = date.succ_opt().unwrap();
                }

                Ok(())
            }

            StatementsMergingStrategy::SparseSingleDaysLastMonth(monthly_statement_availability_delay) => {
                assert!(second.last_date() <= last_date);

                if second.first_date() == first.next_date() {
                    return Ok(());
                }

                if second.days() != 1 {
                    return error("Non-continuous periods");
                }

                let last_month: Month = last_date.into();
                let start_month = if last_date.day() < 1 + monthly_statement_availability_delay {
                    last_month.prev()
                } else {
                    last_month
                };

                let sparse_period = Period::new(
                    start_month.period().first_date(), last_date).unwrap();

                if !sparse_period.contains(first.next_date()) {
                    return error("Non-continuous periods");
                }

                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use matches::assert_matches;
    use rstest::rstest;
    use super::*;

    #[rstest(delay, first, second, last, ok,
        // Previous month: continuous
        case(0, date!(2021, 12, 29), date!(2021, 12, 30), date!(2022, 1, 4), true),
        case(3, date!(2021, 12, 29), date!(2021, 12, 30), date!(2022, 1, 4), true),

        // Previous month: non-continuous
        case(0, date!(2021, 12, 29), date!(2021, 12, 31), date!(2022, 1, 4), false),
        case(3, date!(2021, 12, 29), date!(2021, 12, 31), date!(2022, 1, 4), false),
        case(4, date!(2021, 12, 29), date!(2021, 12, 31), date!(2022, 1, 4), true),
        case(4, date!(2021, 11, 30), date!(2021, 12,  2), date!(2022, 1, 4), true),
        case(4, date!(2021, 11, 30), date!(2021, 12,  1), date!(2022, 1, 4), true),
        case(4, date!(2021, 11, 29), date!(2021, 12,  1), date!(2022, 1, 4), false),

        // Previous month: continuous
        case(0, date!(2021, 12, 30), date!(2021, 12, 31), date!(2022, 1, 4), true),
        case(3, date!(2021, 12, 30), date!(2021, 12, 31), date!(2022, 1, 4), true),

        // Previous month: non-continuous
        case(0, date!(2021, 12, 30), date!(2022,  1,  1), date!(2022, 1, 4), false),
        case(3, date!(2021, 12, 30), date!(2022,  1,  1), date!(2022, 1, 4), false),
        case(4, date!(2021, 12, 30), date!(2022,  1,  1), date!(2022, 1, 4), true),

        // Previous month: continuous
        case(0, date!(2021, 12, 31), date!(2022,  1,  1), date!(2022, 1, 4), true),
        case(3, date!(2021, 12, 31), date!(2022,  1,  1), date!(2022, 1, 4), true),

        // Current month: non-continuous
        case(0, date!(2021, 12, 31), date!(2022,  1,  2), date!(2022, 1, 4), true),
        case(3, date!(2021, 12, 31), date!(2022,  1,  2), date!(2022, 1, 4), true),
        case(4, date!(2021, 12, 31), date!(2022,  1,  2), date!(2022, 1, 4), true),

        // Current month: non-continuous
        case(0, date!(2021, 12, 31), date!(2022,  1,  3), date!(2022, 1, 4), true),
        case(3, date!(2021, 12, 31), date!(2022,  1,  3), date!(2022, 1, 4), true),
        case(4, date!(2021, 12, 31), date!(2022,  1,  3), date!(2022, 1, 4), true),

        // Current month: non-continuous
        case(0, date!(2021, 12, 31), date!(2022,  1,  4), date!(2022, 1, 4), true),
        case(3, date!(2021, 12, 31), date!(2022,  1,  4), date!(2022, 1, 4), true),
        case(4, date!(2021, 12, 31), date!(2022,  1,  4), date!(2022, 1, 4), true),

        // Current month: non-continuous
        case(0, date!(2022,  1,  1), date!(2022,  1,  3), date!(2022, 1, 4), true),
        case(3, date!(2022,  1,  1), date!(2022,  1,  3), date!(2022, 1, 4), true),
        case(4, date!(2022,  1,  1), date!(2022,  1,  3), date!(2022, 1, 4), true),
    )]
    fn sparse_single_days_last_month(delay: u32, first: Date, second: Date, last: Date, ok: bool) {
        let period = |date| Period::new(date, date).unwrap();
        let strategy = StatementsMergingStrategy::SparseSingleDaysLastMonth(delay);

        let result = strategy.validate(period(first), period(second), last);
        if ok {
            assert_matches!(result, Ok(()));
        } else {
            assert_matches!(result, Err(e) if e.to_string().starts_with("Non-continuous periods: "));
        }
    }
}