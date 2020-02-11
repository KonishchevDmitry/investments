use std::{self, fs};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::path::Path;

use chrono::Duration;
use log::{debug, warn};

use crate::brokers::{Broker, BrokerInfo};
use crate::commissions::CommissionCalc;
use crate::config::Config;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets, MultiCurrencyCashAccount};
use crate::formatting;
use crate::quotes::Quotes;
use crate::types::{Date, Decimal, TradeType};
use crate::util;

use self::dividends::{Dividend, DividendAccruals, process_dividend_accruals};
use self::interest::IdleCashInterest;
use self::partial::PartialBrokerStatement;
use self::taxes::{TaxId, TaxAccruals};
use self::trades::{StockBuy, StockSell, StockSellSource};

mod bcs;
mod dividends;
mod ib;
mod interest;
mod open_broker;
mod partial;
mod payments;
mod taxes;
pub mod trades;

#[derive(Debug)]
pub struct BrokerStatement {
    pub broker: BrokerInfo,
    pub period: (Date, Date),

    pub cash_flows: Vec<CashAssets>,
    pub cash_assets: MultiCurrencyCashAccount,
    pub idle_cash_interest: Vec<IdleCashInterest>,

    pub stock_buys: Vec<StockBuy>,
    pub stock_sells: Vec<StockSell>,
    pub dividends: Vec<Dividend>,

    pub open_positions: HashMap<String, u32>,
    instrument_names: HashMap<String, String>,
}

impl BrokerStatement {
    pub fn read(config: &Config, broker: Broker, statement_dir_path: &str) -> GenericResult<BrokerStatement> {
        let statement_reader = match broker {
            Broker::Bcs => bcs::StatementReader::new(config),
            Broker::InteractiveBrokers => ib::StatementReader::new(config),
            Broker::OpenBroker => open_broker::StatementReader::new(config),
        }?;

        let mut file_names = get_statement_files(statement_dir_path, statement_reader.as_ref())
            .map_err(|e| format!("Error while reading {:?}: {}", statement_dir_path, e))?;

        if file_names.is_empty() {
            return Err!("{:?} doesn't contain any broker statement", statement_dir_path);
        }

        file_names.sort();

        let mut statements = Vec::new();

        for file_name in &file_names {
            let path = Path::new(statement_dir_path).join(file_name);
            let path = path.to_str().unwrap();

            let statement = statement_reader.read(path).map_err(|e| format!(
                "Error while reading {:?} broker statement: {}", path, e))?;

            statements.push(statement);
        }

        let joint_statement = BrokerStatement::new_from(statements)?;
        debug!("{:#?}", joint_statement);
        Ok(joint_statement)
    }

    fn new_from(mut statements: Vec<PartialBrokerStatement>) -> GenericResult<BrokerStatement> {
        statements.sort_by(|a, b| a.period.unwrap().0.cmp(&b.period.unwrap().0));

        let mut statement = BrokerStatement::new_empty_from(statements.first().unwrap())?;
        let mut dividend_accruals = HashMap::new();
        let mut tax_accruals = HashMap::new();

        for mut partial in statements.drain(..) {
            for (dividend_id, accruals) in partial.dividend_accruals.drain() {
                dividend_accruals.entry(dividend_id)
                    .and_modify(|existing: &mut DividendAccruals| existing.merge(&accruals))
                    .or_insert(accruals);
            }

            for (tax_id, accruals) in partial.tax_accruals.drain() {
                tax_accruals.entry(tax_id)
                    .and_modify(|existing: &mut TaxAccruals| existing.merge(&accruals))
                    .or_insert(accruals);
            }

            statement.merge(partial).map_err(|e| format!(
                "Failed to merge broker statements: {}", e))?;
        }

        for (dividend_id, accruals) in dividend_accruals {
            if let Some(dividend) = process_dividend_accruals(dividend_id, accruals, &mut tax_accruals)? {
                statement.dividends.push(dividend);
            }
        }

        if !tax_accruals.is_empty() {
            let taxes = tax_accruals.keys()
                .map(|tax: &TaxId| format!(
                    "* {date}: {issuer}", date=formatting::format_date(tax.date),
                    issuer=tax.issuer))
                .collect::<Vec<_>>()
                .join("\n");

            return Err!("Unable to find origin operations for the following taxes:\n{}", taxes);
        }

        statement.sort()?;
        statement.validate()?;
        statement.process_trades()?;

        Ok(statement)
    }

    fn new_empty_from(statement: &PartialBrokerStatement) -> GenericResult<BrokerStatement> {
        let mut period = statement.get_period()?;
        period.1 = period.0;

        if statement.get_starting_assets()? {
            return Err!("Invalid broker statement period: It has a non-zero starting assets");
        }

        Ok(BrokerStatement {
            broker: statement.broker.clone(),
            period: period,

            cash_flows: Vec::new(),
            cash_assets: MultiCurrencyCashAccount::new(),
            idle_cash_interest: Vec::new(),

            stock_buys: Vec::new(),
            stock_sells: Vec::new(),
            dividends: Vec::new(),

            open_positions: HashMap::new(),
            instrument_names: HashMap::new(),
        })
    }

    pub fn check_date(&self) {
        let date = self.period.1 - Duration::days(1);
        let days = (util::today() - date).num_days();
        let months = Decimal::from(days) / dec!(30);

        if months >= dec!(1) {
            warn!("The broker statement is {} months old and may be outdated.",
                  util::round(months, 1));
        }
    }

    pub fn get_instrument_name(&self, symbol: &str) -> String {
        if let Some(name) = self.instrument_names.get(symbol) {
            format!("{} ({})", name, symbol)
        } else {
            symbol.to_owned()
        }
    }

    pub fn batch_quotes(&self, quotes: &Quotes) {
        for symbol in self.open_positions.keys() {
            quotes.batch(&symbol);
        }
    }

    pub fn emulate_sell(
        &mut self, symbol: &str, quantity: u32, price: Cash, commission_calc: &mut CommissionCalc
    ) -> EmptyResult {
        let today = util::today();

        let conclusion_date = today;
        let execution_date = match self.stock_sells.last() {
            Some(last_trade) if last_trade.execution_date > today => last_trade.execution_date,
            _ => today,
        };

        let commission = commission_calc.add_trade(
            conclusion_date, TradeType::Sell, quantity, price)?;

        let stock_cell = StockSell::new(
            symbol, quantity, price, commission, conclusion_date, execution_date, true);

        if let Entry::Occupied(mut open_position) = self.open_positions.entry(symbol.to_owned()) {
            let available = *open_position.get();

            if available == quantity {
                open_position.remove();
            } else if available > quantity {
                *open_position.get_mut() -= quantity;
            } else {
                return Err!("The portfolio has not enough open positions for {}", symbol);
            }
        } else {
            return Err!("The portfolio has no open {} position", symbol);
        }

        self.stock_sells.push(stock_cell);
        self.cash_assets.deposit(price * quantity);
        self.cash_assets.withdraw(commission);

        Ok(())
    }

    pub fn emulate_commissions(&mut self, commission_calc: CommissionCalc) -> MultiCurrencyCashAccount {
        let mut total = MultiCurrencyCashAccount::new();

        for &commission in commission_calc.calculate().values() {
            self.cash_assets.withdraw(commission);
            total.deposit(commission);
        }

        total
    }

    pub fn process_trades(&mut self) -> EmptyResult {
        let stock_buys_num = self.stock_buys.len();
        let mut stock_buys = Vec::with_capacity(stock_buys_num);
        let mut unsold_stock_buys: HashMap<String, Vec<StockBuy>> = HashMap::new();

        for stock_buy in self.stock_buys.drain(..).rev() {
            if stock_buy.is_sold() {
                stock_buys.push(stock_buy);
                continue;
            }

            let symbol_buys = match unsold_stock_buys.get_mut(&stock_buy.symbol) {
                Some(symbol_buys) => symbol_buys,
                None => {
                    unsold_stock_buys.insert(stock_buy.symbol.clone(), Vec::new());
                    unsold_stock_buys.get_mut(&stock_buy.symbol).unwrap()
                },
            };

            symbol_buys.push(stock_buy);
        }

        for stock_sell in &mut self.stock_sells {
            if stock_sell.is_processed() {
                continue;
            }

            let mut remaining_quantity = stock_sell.quantity;
            let mut sources = Vec::new();

            let symbol_buys = unsold_stock_buys.get_mut(&stock_sell.symbol).ok_or_else(|| format!(
                "Error while processing {} position closing: There are no open positions for it",
                stock_sell.symbol
            ))?;

            while remaining_quantity > 0 {
                let mut stock_buy = symbol_buys.pop().ok_or_else(|| format!(
                    "Error while processing {} position closing: There are no open positions for it",
                    stock_sell.symbol
                ))?;

                let sell_quantity = std::cmp::min(remaining_quantity, stock_buy.get_unsold());
                assert!(sell_quantity > 0);

                let commission = (stock_buy.commission / stock_buy.quantity * sell_quantity).round();

                sources.push(StockSellSource {
                    quantity: sell_quantity,
                    price: stock_buy.price,
                    commission: commission,

                    conclusion_date: stock_buy.conclusion_date,
                    execution_date: stock_buy.execution_date,
                });

                remaining_quantity -= sell_quantity;
                stock_buy.sell(sell_quantity);

                if stock_buy.is_sold() {
                    stock_buys.push(stock_buy);
                } else {
                    symbol_buys.push(stock_buy);
                }
            }

            stock_sell.process(sources);
        }

        for (_, mut symbol_buys) in unsold_stock_buys.drain() {
            stock_buys.extend(symbol_buys.drain(..));
        }
        drop(unsold_stock_buys);

        assert_eq!(stock_buys.len(), stock_buys_num);
        self.stock_buys = stock_buys;
        self.sort_stock_buys()?;

        self.validate_open_positions()?;

        Ok(())
    }

    pub fn merge_symbols(&mut self, symbols_to_merge: &HashMap<String, HashSet<String>>) -> EmptyResult {
        assert!(self.open_positions.is_empty());
        assert!(!self.stock_buys.iter().any(|stock_buy| !stock_buy.is_sold()));
        assert!(!self.stock_sells.iter().any(|stock_sell| !stock_sell.is_processed()));

        let mut symbol_mapping: HashMap<&String, &String> = HashMap::new();

        for (master_symbol, slave_symbols) in symbols_to_merge {
            for slave_symbol in slave_symbols {
                symbol_mapping.insert(slave_symbol, master_symbol);
            }
        }

        for &symbol in symbol_mapping.keys() {
            if self.instrument_names.remove(symbol).is_none() {
                return Err!("The broker statement has no any activity for {:?} symbol", symbol);
            }
        }

        for stock_buy in &mut self.stock_buys {
            if let Some(&symbol) = symbol_mapping.get(&stock_buy.symbol) {
                stock_buy.symbol = symbol.clone();
            }
        }

        for stock_sell in &mut self.stock_sells {
            if let Some(&symbol) = symbol_mapping.get(&stock_sell.symbol) {
                stock_sell.symbol = symbol.clone();
            }
        }

        for dividend in &mut self.dividends {
            if let Some(&issuer) = symbol_mapping.get(&dividend.issuer) {
                dividend.issuer = issuer.clone();
            }
        }

        Ok(())
    }

    fn merge(&mut self, mut statement: PartialBrokerStatement) -> EmptyResult {
        let period = statement.get_period()?;

        if statement.broker.allow_sparse_broker_statements {
            if period.0 < self.period.1 {
                return Err!("Overlapping periods: {}, {}",
                formatting::format_period(self.period.0, self.period.1),
                formatting::format_period(period.0, period.1));
            }
        } else {
            if period.0 != self.period.1 {
                return Err!("Non-continuous periods: {}, {}",
                formatting::format_period(self.period.0, self.period.1),
                formatting::format_period(period.0, period.1));
            }
        }

        self.period.1 = period.1;

        self.cash_flows.extend(statement.cash_flows.drain(..));
        self.cash_assets = statement.cash_assets;
        self.idle_cash_interest.extend(statement.idle_cash_interest.drain(..));

        self.stock_buys.extend(statement.stock_buys.drain(..));
        self.stock_sells.extend(statement.stock_sells.drain(..));
        self.dividends.extend(statement.dividends.drain(..));

        self.open_positions = statement.open_positions;
        self.instrument_names.extend(statement.instrument_names.drain());

        Ok(())
    }

    fn sort(&mut self) -> EmptyResult {
        self.cash_flows.sort_by_key(|cash_flow| cash_flow.date);
        self.idle_cash_interest.sort_by_key(|interest| interest.date);
        self.dividends.sort_by(|a, b| (a.date, &a.issuer).cmp(&(b.date, &b.issuer)));
        self.sort_stock_buys()?;
        self.sort_stock_sells()?;
        Ok(())
    }

    fn sort_stock_buys(&mut self) -> EmptyResult {
        self.stock_buys.sort_by_key(|trade| (trade.conclusion_date, trade.execution_date));

        let mut prev_execution_date = None;

        for stock_buy in &self.stock_buys {
            if let Some(prev_execution_date) = prev_execution_date {
                if stock_buy.execution_date < prev_execution_date {
                    return Err!("Got an unexpected execution order for buy trades");
                }
            }

            prev_execution_date = Some(stock_buy.execution_date);
        }

        Ok(())
    }

    fn sort_stock_sells(&mut self) -> EmptyResult {
        self.stock_sells.sort_by_key(|trade| (trade.conclusion_date, trade.execution_date));

        let mut prev_execution_date = None;

        for stock_sell in &self.stock_sells {
            if let Some(prev_execution_date) = prev_execution_date {
                if stock_sell.execution_date < prev_execution_date {
                    return Err!("Got an unexpected execution order for sell trades");
                }
            }

            prev_execution_date = Some(stock_sell.execution_date);
        }

        Ok(())
    }

    fn validate(&self) -> EmptyResult {
        let min_date = self.period.0;
        let max_date = self.period.1 - Duration::days(1);
        let validate_date = |name, first_date, last_date| -> EmptyResult {
            if first_date < min_date {
                return Err!("Got a {} outside of statement period: {}",
                    name, formatting::format_date(first_date));
            }

            if last_date > max_date {
                return Err!("Got a {} outside of statement period: {}",
                    name, formatting::format_date(first_date));
            }

            Ok(())
        };

        if !self.cash_flows.is_empty() {
            let first_date = self.cash_flows.first().unwrap().date;
            let last_date = self.cash_flows.last().unwrap().date;
            validate_date("cash flow", first_date, last_date)?;
        }

        if !self.idle_cash_interest.is_empty() {
            let first_date = self.idle_cash_interest.first().unwrap().date;
            let last_date = self.idle_cash_interest.last().unwrap().date;
            validate_date("idle cash interest", first_date, last_date)?;
        }

        if !self.stock_buys.is_empty() {
            let first_date = self.stock_buys.first().unwrap().conclusion_date;
            let last_date = self.stock_buys.last().unwrap().conclusion_date;
            validate_date("stock buy", first_date, last_date)?;
        }

        if !self.stock_sells.is_empty() {
            let first_date = self.stock_sells.first().unwrap().conclusion_date;
            let last_date = self.stock_sells.last().unwrap().conclusion_date;
            validate_date("stock sell", first_date, last_date)?;
        }

        if !self.dividends.is_empty() {
            let first_date = self.dividends.first().unwrap().date;
            let last_date = self.dividends.last().unwrap().date;
            validate_date("dividend", first_date, last_date)?;
        }

        Ok(())
    }

    fn validate_open_positions(&self) -> EmptyResult {
        let mut open_positions = HashMap::new();

        for stock_buy in &self.stock_buys {
            if stock_buy.is_sold() {
                continue;
            }

            let quantity = stock_buy.get_unsold();

            if let Some(position) = open_positions.get_mut(&stock_buy.symbol) {
                *position += quantity;
            } else {
                open_positions.insert(stock_buy.symbol.clone(), quantity);
            }
        }

        if open_positions != self.open_positions {
            return Err!("The calculated open positions don't match declared ones in the statement");
        }

        Ok(())
    }
}

fn get_statement_files(
    statement_dir_path: &str, statement_reader: &dyn BrokerStatementReader
) -> GenericResult<Vec<String>> {
    let mut file_names = Vec::new();

    for entry in fs::read_dir(statement_dir_path)? {
        let entry = entry?;

        let path = entry.path();
        let path = path.to_str().ok_or_else(|| format!(
            "Got an invalid path: {:?}", path.to_string_lossy()))?;

        if !statement_reader.is_statement(&path)? {
            continue;
        }

        let file_name = entry.file_name().into_string().map_err(|file_name| format!(
            "Got an invalid file name: {:?}", file_name.to_string_lossy()))?;
        file_names.push(file_name);
    }

    Ok(file_names)
}

pub trait BrokerStatementReader {
    fn is_statement(&self, path: &str) -> GenericResult<bool>;
    fn read(&self, path: &str) -> GenericResult<PartialBrokerStatement>;
}