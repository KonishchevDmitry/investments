use std::collections::HashMap;

use crate::brokers::BrokerInfo;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{CashAssets, MultiCurrencyCashAccount};
use crate::formatting;
use crate::types::Date;

use super::{Dividend, StockBuy, StockSell};
use super::dividends::DividendWithoutPaidTax;
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

    pub dividends_without_paid_tax: Vec<DividendWithoutPaidTax>,
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

            dividends_without_paid_tax: Vec::new(),
            tax_changes: HashMap::new(), // FIXME: Fill from statements

            open_positions: HashMap::new(),
            instrument_names: HashMap::new(),
        }
    }

    pub fn set_period(&mut self, period: (Date, Date)) -> EmptyResult {
        set_option("statement period", &mut self.period, period)
    }

    pub fn get_period(&self) -> GenericResult<(Date, Date)> {
        get_option("statement period", self.period)
    }

    pub fn set_starting_assets(&mut self, exists: bool) -> EmptyResult {
        set_option("starting assets", &mut self.starting_assets, exists)
    }

    pub fn get_starting_assets(&self) -> GenericResult<bool> {
        get_option("starting assets", self.starting_assets)
    }

    pub fn validate(self) -> GenericResult<PartialBrokerStatement> {
        let period = self.get_period()?;
        if period.0 >= period.1 {
            return Err!("Invalid statement period: {}",
                        formatting::format_period(period.0, period.1));
        }

        self.get_starting_assets()?;

        if self.cash_assets.is_empty() {
            return Err!("Unable to find any information about current cash assets");
        }

        Ok(self)
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