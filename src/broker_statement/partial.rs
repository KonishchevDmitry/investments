use std::collections::HashMap;

use crate::brokers::BrokerInfo;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{CashAssets, MultiCurrencyCashAccount};
use crate::types::Date;

use super::{BrokerStatement, Dividend, StockBuy, StockSell};
use super::taxes::{TaxId, TaxChanges};

pub struct PartialBrokerStatement {
    pub broker: BrokerInfo,
    pub period: Option<(Date, Date)>,

    pub starting_assets: Option<bool>,
    pub cash_flows: Vec<CashAssets>,
    pub cash_assets: MultiCurrencyCashAccount,

    pub stock_buys: Vec<StockBuy>,
    pub stock_sells: Vec<StockSell>,
    pub dividends: Vec<Dividend>,
    pub tax_changes: HashMap<TaxId, TaxChanges>,

    pub open_positions: HashMap<String, u32>,
    pub instrument_names: HashMap<String, String>,
}

impl PartialBrokerStatement {
    pub fn new(broker: BrokerInfo) -> PartialBrokerStatement {
        PartialBrokerStatement {
            broker: broker,
            period: None,

            starting_assets: None,
            cash_flows: Vec::new(),
            cash_assets: MultiCurrencyCashAccount::new(),

            stock_buys: Vec::new(),
            stock_sells: Vec::new(),
            dividends: Vec::new(),
            tax_changes: HashMap::new(), // FIXME: Fill from statements

            open_positions: HashMap::new(),
            instrument_names: HashMap::new(),
        }
    }

    pub fn set_period(&mut self, period: (Date, Date)) -> EmptyResult {
        set_option("statement period", &mut self.period, period)
    }

    pub fn set_starting_assets(&mut self, exists: bool) -> EmptyResult {
        set_option("starting assets", &mut self.starting_assets, exists)
    }

    // FIXME: To validate
    pub fn get(self) -> GenericResult<BrokerStatement> {
        let mut statement = BrokerStatement {
            broker: self.broker,
            period: get_option("statement period", self.period)?,

            starting_assets: get_option("starting assets", self.starting_assets)?,
            cash_flows: self.cash_flows,
            cash_assets: self.cash_assets,

            stock_buys: self.stock_buys,
            stock_sells: self.stock_sells,
            dividends: self.dividends,

            open_positions: self.open_positions,
            instrument_names: self.instrument_names,
        };
        statement.validate()?;
        Ok(statement)
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