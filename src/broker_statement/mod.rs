use core::{EmptyResult, GenericResult};
use currency::Cash;
use types::Date;

pub mod ib;

#[derive(Debug)]
pub struct BrokerStatement {
    period: (Date, Date),
    deposits: Vec<Transaction>,
}

struct BrokerStatementBuilder {
    period: Option<(Date, Date)>,
    deposits: Vec<Transaction>,
}

impl BrokerStatementBuilder {
    fn new() -> BrokerStatementBuilder {
        BrokerStatementBuilder {
            period: None,
            deposits: Vec::new(),
        }
    }

    fn set_period(&mut self, period: (Date, Date)) -> EmptyResult {
        set_option("statement period", &mut self.period, period)
    }

    fn get(self) -> GenericResult<BrokerStatement> {
        return Ok(BrokerStatement {
            period: get_option("statement period", self.period)?,
            deposits: self.deposits,
        })
    }
}

fn get_option<T>(name: &str, option: Option<T>) -> GenericResult<T> {
    match option {
        Some(value) => Ok(value),
        Node => Err!("{} is missing", name)
    }
}

fn set_option<T>(name: &str, option: &mut Option<T>, value: T) -> EmptyResult {
    if option.is_some() {
        return Err!("Duplicate {}", name);
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