use chrono::NaiveDate;

use core::EmptyResult;

pub mod ib;

#[derive(Debug)]
struct StatementBuilder {
    period: Option<(NaiveDate, NaiveDate)>,
}

impl StatementBuilder {
    fn new() -> StatementBuilder {
        StatementBuilder {
            period: None,
        }
    }

    fn set_period(&mut self, period: (NaiveDate, NaiveDate)) -> EmptyResult {
        set_option("period", &mut self.period, period)
    }
}

fn set_option<T>(name: &str, option: &mut Option<T>, value: T) -> EmptyResult {
    if option.is_some() {
        return Err!("Duplicate statement {}", name);
    }
    *option = Some(value);
    Ok(())
}