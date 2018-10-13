use core::{EmptyResult, GenericResult};
use currency::{Cash, CashAssets};
use types::Date;

pub mod ib;

#[derive(Debug)]
pub struct BrokerStatement {
    pub period: (Date, Date),
    pub deposits: Vec<CashAssets>,
    pub total_value: Cash,
}

struct BrokerStatementBuilder {
    period: Option<(Date, Date)>,
    deposits: Vec<CashAssets>,
    total_value: Option<Cash>,
}

impl BrokerStatementBuilder {
    fn new() -> BrokerStatementBuilder {
        BrokerStatementBuilder {
            period: None,
            deposits: Vec::new(),
            total_value: None,
        }
    }

    fn set_period(&mut self, period: (Date, Date)) -> EmptyResult {
        set_option("statement period", &mut self.period, period)
    }

    fn get(self) -> GenericResult<BrokerStatement> {
        return Ok(BrokerStatement {
            period: get_option("statement period", self.period)?,
            deposits: self.deposits,
            total_value: get_option("total value", self.total_value)?,
        })
    }
}

fn get_option<T>(name: &str, option: Option<T>) -> GenericResult<T> {
    match option {
        Some(value) => Ok(value),
        None => Err!("{} is missing", name)
    }
}

fn set_option<T>(name: &str, option: &mut Option<T>, value: T) -> EmptyResult {
    if option.is_some() {
        return Err!("Duplicate {}", name);
    }
    *option = Some(value);
    Ok(())
}