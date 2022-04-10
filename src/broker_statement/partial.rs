use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::rc::Rc;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets, MultiCurrencyCashAccount};
use crate::exchanges::{Exchange, Exchanges};
use crate::instruments::{InstrumentId, InstrumentInfo};
use crate::time::{Date, Period};
use crate::types::Decimal;
use crate::util::{DecimalRestrictions, validate_named_decimal};

use super::cash_flows::CashFlow;
use super::corporate_actions::CorporateAction;
use super::dividends::{DividendId, DividendAccruals};
use super::fees::Fee;
use super::grants::StockGrant;
use super::interest::IdleCashInterest;
use super::trades::{ForexTrade, StockBuy, StockSell};
use super::taxes::{TaxId, TaxAccruals, TaxWithholding};

pub type PartialBrokerStatementRc = Rc<RefCell<PartialBrokerStatement>>;

pub struct PartialBrokerStatement {
    pub period: Option<Period>,

    pub has_starting_assets: Option<bool>,
    pub deposits_and_withdrawals: Vec<CashAssets>,
    pub cash_flows: Vec<CashFlow>,
    pub fees: Vec<Fee>,
    pub idle_cash_interest: Vec<IdleCashInterest>,
    pub tax_agent_withholdings: Vec<TaxWithholding>,

    pub exchanges: Exchanges,
    pub forex_trades: Vec<ForexTrade>,
    pub stock_buys: Vec<StockBuy>,
    pub stock_sells: Vec<StockSell>,

    pub dividend_accruals: HashMap<DividendId, DividendAccruals>,
    pub tax_accruals: HashMap<TaxId, TaxAccruals>,

    pub stock_grants: Vec<StockGrant>,
    pub corporate_actions: Vec<CorporateAction>,

    // Please note that some brokers (Firstrade) provide this information only for the last
    // statement (current date).
    pub assets: NetAssets,
    pub open_positions: HashMap<String, Decimal>,
    pub instrument_info: InstrumentInfo,
}

pub struct NetAssets {
    pub cash: Option<MultiCurrencyCashAccount>,
    pub other: Option<Cash>, // Supported only for some brokers
}

impl PartialBrokerStatement {
    pub fn new(exchanges: &[Exchange], zero_cash_assets: bool) -> PartialBrokerStatement {
        PartialBrokerStatement {
            period: None,

            has_starting_assets: None,
            deposits_and_withdrawals: Vec::new(),
            cash_flows: Vec::new(),
            fees: Vec::new(),
            idle_cash_interest: Vec::new(),
            tax_agent_withholdings: Vec::new(),

            exchanges: Exchanges::new(exchanges),
            forex_trades: Vec::new(),
            stock_buys: Vec::new(),
            stock_sells: Vec::new(),

            dividend_accruals: HashMap::new(),
            tax_accruals: HashMap::new(),

            stock_grants: Vec::new(),
            corporate_actions: Vec::new(),

            assets: NetAssets {
                cash: if zero_cash_assets {
                    Some(MultiCurrencyCashAccount::new())
                } else {
                    None
                },
                other: None
            },
            open_positions: HashMap::new(),
            instrument_info: InstrumentInfo::new(),
        }
    }

    pub fn new_rc(exchanges: &[Exchange], zero_cash_assets: bool) -> PartialBrokerStatementRc {
        Rc::new(RefCell::new(PartialBrokerStatement::new(exchanges, zero_cash_assets)))
    }

    pub fn set_period(&mut self, period: Period) -> EmptyResult {
        set_option("statement period", &mut self.period, period)
    }

    pub fn get_period(&self) -> GenericResult<Period> {
        get_option("statement period", self.period)
    }

    pub fn set_has_starting_assets(&mut self, exists: bool) -> EmptyResult {
        set_option("has starting assets", &mut self.has_starting_assets, exists)
    }

    pub fn get_has_starting_assets(&self) -> GenericResult<bool> {
        get_option("has starting assets", self.has_starting_assets)
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

    pub fn dividend_accruals(&mut self, date: Date, issuer: InstrumentId, strict: bool) -> &mut DividendAccruals {
        self.dividend_accruals.entry(DividendId::new(date, issuer))
            .or_insert_with(|| DividendAccruals::new(strict))
    }

    pub fn tax_accruals(&mut self, date: Date, issuer: InstrumentId, strict: bool) -> &mut TaxAccruals {
        self.tax_accruals.entry(TaxId::new(date, issuer))
            .or_insert_with(|| TaxAccruals::new(strict))
    }

    pub fn validate(self) -> GenericResult<PartialBrokerStatement> {
        self.get_period()?;
        self.get_has_starting_assets()?;
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