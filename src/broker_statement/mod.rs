use std::collections::HashMap;

use core::{EmptyResult, GenericResult};
use currency::{self, Cash, CashAssets};
use currency::converter::CurrencyConverter;
use regulations::Country;
use types::{Date, Decimal};

pub mod ib;

#[derive(Debug)]
pub struct BrokerInfo {
    pub name: &'static str,
}

// TODO: Take care of stock splitting
#[derive(Debug)]
pub struct BrokerStatement {
    pub broker: BrokerInfo,
    pub period: (Date, Date),
    pub deposits: Vec<CashAssets>,
    pub dividends: Vec<Dividend>,
    pub instrument_names: HashMap<String, String>,
    pub total_value: Cash,
}

impl BrokerStatement {
    pub fn get_instrument_name(&self, ticker: &str) -> GenericResult<String> {
        let name = self.instrument_names.get(ticker).ok_or_else(|| format!(
            "Unable to find {:?} instrument name in the broker statement", ticker))?;
        Ok(format!("{} ({})", name, ticker))
    }
}

struct BrokerStatementBuilder {
    broker: BrokerInfo,
    period: Option<(Date, Date)>,
    deposits: Vec<CashAssets>,
    dividends: Vec<Dividend>,
    instrument_names: HashMap<String, String>,
    total_value: Option<Cash>,
}

impl BrokerStatementBuilder {
    fn new(broker: BrokerInfo) -> BrokerStatementBuilder {
        BrokerStatementBuilder {
            broker: broker,
            period: None,
            deposits: Vec::new(),
            dividends: Vec::new(),
            instrument_names: HashMap::new(),
            total_value: None,
        }
    }

    fn set_period(&mut self, period: (Date, Date)) -> EmptyResult {
        set_option("statement period", &mut self.period, period)
    }

    fn get(self) -> GenericResult<BrokerStatement> {
        let statement = BrokerStatement {
            broker: self.broker,
            period: get_option("statement period", self.period)?,
            deposits: self.deposits,
            dividends: self.dividends,
            instrument_names: self.instrument_names,
            total_value: get_option("total value", self.total_value)?,
        };
        debug!("{:#?}", statement);
        return Ok(statement)
    }
}

#[derive(Debug)]
pub struct Dividend {
    pub date: Date,
    pub issuer: String,
    pub amount: Cash,
    pub paid_tax: Cash,
}

impl Dividend {
    pub fn tax_to_pay(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Decimal> {
        let amount = converter.convert_to(self.date, self.amount, country.currency)?;
        let tax_amount = currency::round(amount * country.tax_rate);
        let paid_tax = currency::round(converter.convert_to(
            self.date, self.paid_tax, country.currency)?);

        Ok(if paid_tax < tax_amount {
            tax_amount - paid_tax
        } else {
            dec!(0)
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
