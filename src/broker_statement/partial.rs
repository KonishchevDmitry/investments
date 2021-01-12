use std::collections::HashMap;
use std::collections::hash_map::Entry;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::{CashAssets, MultiCurrencyCashAccount};
use crate::formatting;
use crate::types::{Date, Decimal};
use crate::util::{DecimalRestrictions, validate_named_decimal};

use super::corporate_actions::CorporateAction;
use super::dividends::{Dividend, DividendId, DividendAccruals};
use super::fees::Fee;
use super::interest::IdleCashInterest;
use super::trades::{ForexTrade, StockBuy, StockSell};
use super::taxes::{TaxId, TaxAccruals, TaxWithholding};

pub struct PartialBrokerStatement {
    pub period: Option<(Date, Date)>,

    pub starting_assets: Option<bool>,
    pub cash_flows: Vec<CashAssets>,
    pub fees: Vec<Fee>,
    pub idle_cash_interest: Vec<IdleCashInterest>,
    pub tax_agent_withholdings: Vec<TaxWithholding>,

    pub forex_trades: Vec<ForexTrade>,
    pub stock_buys: Vec<StockBuy>,
    pub stock_sells: Vec<StockSell>,
    pub dividends: Vec<Dividend>,

    pub instrument_names: HashMap<String, String>,
    pub corporate_actions: Vec<CorporateAction>,
    pub dividend_accruals: HashMap<DividendId, DividendAccruals>,
    pub tax_accruals: HashMap<TaxId, TaxAccruals>,

    // Please note that some brokers (Firstrade) provide this information only for the last
    // statement (current date).
    pub cash_assets: Option<MultiCurrencyCashAccount>,
    pub open_positions: HashMap<String, Decimal>,
}

impl PartialBrokerStatement {
    pub fn new(zero_cash_assets: bool) -> PartialBrokerStatement {
        PartialBrokerStatement {
            period: None,

            starting_assets: None,
            cash_flows: Vec::new(),
            fees: Vec::new(),
            idle_cash_interest: Vec::new(),
            tax_agent_withholdings: Vec::new(),

            forex_trades: Vec::new(),
            stock_buys: Vec::new(),
            stock_sells: Vec::new(),
            dividends: Vec::new(),

            instrument_names: HashMap::new(),
            corporate_actions: Vec::new(),
            dividend_accruals: HashMap::new(),
            tax_accruals: HashMap::new(),

            cash_assets: if zero_cash_assets {
                Some(MultiCurrencyCashAccount::new())
            } else {
                None
            },
            open_positions: HashMap::new(),
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

    pub fn add_open_position(&mut self, symbol: &str, quantity: Decimal) -> EmptyResult {
        validate_named_decimal(
            &format!("{} open position", symbol), quantity, DecimalRestrictions::StrictlyPositive)?;

        match self.open_positions.entry(symbol.to_owned()) {
            Entry::Vacant(entry) => entry.insert(quantity),
            Entry::Occupied(_) => return Err!("Got a duplicated open position for {}", symbol),
        };

        Ok(())
    }

    pub fn validate(self) -> GenericResult<PartialBrokerStatement> {
        let period = self.get_period()?;
        if period.0 >= period.1 {
            return Err!("Invalid statement period: {}", formatting::format_period(period));
        }

        self.get_starting_assets()?;

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