use std::collections::HashMap;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::{CashAssets, MultiCurrencyCashAccount};
use crate::formatting;
use crate::types::Date;

use super::dividends::{Dividend, DividendId, DividendAccruals};
use super::fees::Fee;
use super::interest::IdleCashInterest;
use super::trades::{ForexTrade, StockBuy, StockSell};
use super::taxes::{TaxId, TaxAccruals};

pub struct PartialBrokerStatement {
    pub period: Option<(Date, Date)>,

    pub starting_assets: Option<bool>,
    pub cash_flows: Vec<CashAssets>,
    pub cash_assets: MultiCurrencyCashAccount,

    pub fees: Vec<Fee>,
    pub idle_cash_interest: Vec<IdleCashInterest>,

    pub forex_trades: Vec<ForexTrade>,
    pub stock_buys: Vec<StockBuy>,
    pub stock_sells: Vec<StockSell>,
    pub dividends: Vec<Dividend>,

    pub dividend_accruals: HashMap<DividendId, DividendAccruals>,
    pub tax_accruals: HashMap<TaxId, TaxAccruals>,

    pub open_positions: HashMap<String, u32>,
    pub instrument_names: HashMap<String, String>,
}

impl PartialBrokerStatement {
    pub fn new() -> PartialBrokerStatement {
        PartialBrokerStatement {
            period: None,

            starting_assets: None,
            cash_flows: Vec::new(),
            cash_assets: MultiCurrencyCashAccount::new(),

            fees: Vec::new(),
            idle_cash_interest: Vec::new(),

            forex_trades: Vec::new(),
            stock_buys: Vec::new(),
            stock_sells: Vec::new(),
            dividends: Vec::new(),

            dividend_accruals: HashMap::new(),
            tax_accruals: HashMap::new(),

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