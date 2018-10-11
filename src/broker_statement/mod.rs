use core::{EmptyResult, GenericResult};
use currency::CacheAssets;
use types::Date;

pub mod ib;

#[derive(Debug)]
pub struct BrokerStatement {
    period: (Date, Date),
    deposits: Vec<CacheAssets>,
}

struct BrokerStatementBuilder {
    period: Option<(Date, Date)>,
    deposits: Vec<CacheAssets>,
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