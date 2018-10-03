use chrono::NaiveDate;

use core::EmptyResult;
use currency::Cash;
use types::Date;

pub mod ib;

#[derive(Debug)]
struct StatementBuilder {
    period: Option<(NaiveDate, NaiveDate)>,
    deposits: Vec<Transaction>,
}

impl StatementBuilder {
    fn new() -> StatementBuilder {
        StatementBuilder {
            period: None,
            deposits: Vec::new(),
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

#[derive(Debug)]
struct Transaction {
    date: Date,
    amount: Cash,
}

impl Transaction {
    fn new(date: Date, amount: Cash) -> Transaction {
        Transaction {date, amount}
    }
}