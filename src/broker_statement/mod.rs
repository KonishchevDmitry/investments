use std::{self, fs};
use std::collections::HashMap;
use std::path::Path;

use chrono::Duration;
use log::{debug, warn};

use crate::brokers::BrokerInfo;
use crate::config::{Config, Broker};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::quotes::Quotes;
use crate::localities::Country;
use crate::types::{Date, Decimal};
use crate::util;

mod ib;
mod open_broker;

#[derive(Debug)]
pub struct BrokerStatement {
    pub broker: BrokerInfo,
    pub period: (Date, Date),

    starting_assets: bool,
    pub cash_flows: Vec<CashAssets>,
    pub cash_assets: MultiCurrencyCashAccount,

    pub stock_buys: Vec<StockBuy>,
    pub stock_sells: Vec<StockSell>,
    pub dividends: Vec<Dividend>,

    pub open_positions: HashMap<String, u32>,
    instrument_names: HashMap<String, String>,
}

impl BrokerStatement {
    pub fn read(config: &Config, broker: Broker, statement_dir_path: &str) -> GenericResult<BrokerStatement> {
        let statement_reader = match broker {
            Broker::InteractiveBrokers => ib::StatementReader::new(config),
            Broker::OpenBroker => open_broker::StatementReader::new(config),
        }?;

        let mut file_names = get_statement_files(statement_dir_path, statement_reader.as_ref())
            .map_err(|e| format!("Error while reading {:?}: {}", statement_dir_path, e))?;
        file_names.sort();

        let mut statements = Vec::new();

        for file_name in &file_names {
            let path = Path::new(statement_dir_path).join(file_name);
            let path = path.to_str().unwrap();
            let statement = statement_reader.read(path).map_err(|e| format!(
                "Error while reading {:?} broker statement: {}", path, e))?;
            statements.push(statement);
        }

        if statements.is_empty() {
            return Err!("{:?} doesn't contain any broker statement", statement_dir_path);
        }
        statements.sort_by(|a, b| b.period.0.cmp(&a.period.0));

        let mut joint_statement = statements.pop().unwrap();
        if joint_statement.starting_assets {
            return Err!("Invalid broker statement period: It has a non-zero starting assets");
        }

        for statement in statements.drain(..).rev() {
            joint_statement.merge(statement).map_err(|e| format!(
                "Failed to merge broker statements: {}", e))?;
        }

        joint_statement.process_trades(false)?;

        debug!("{:#?}", joint_statement);
        Ok(joint_statement)
    }

    pub fn check_date(&self) {
        let date = self.period.1 - Duration::days(1);
        let days = (util::today() - date).num_days();
        let months = Decimal::from(days) / dec!(30);

        if months >= dec!(1) {
            warn!("The broker statement is {} months old and may be outdated.",
                  util::round_to(months, 1));
        }
    }

    pub fn get_instrument_name(&self, symbol: &str) -> GenericResult<String> {
        let name = self.instrument_names.get(symbol).ok_or_else(|| format!(
            "Unable to find {:?} instrument name in the broker statement", symbol))?;
        Ok(format!("{} ({})", name, symbol))
    }

    pub fn batch_quotes(&self, quotes: &mut Quotes) {
        for symbol in self.instrument_names.keys() {
            quotes.batch(&symbol);
        }
    }

    pub fn emulate_sellout(&mut self, quotes: &mut Quotes) -> EmptyResult {
        let today = util::today();

        let conclusion_date = today;
        let execution_date = match self.stock_sells.last() {
            Some(last_trade) if last_trade.execution_date > today => last_trade.execution_date,
            _ => today,
        };

        for (symbol, quantity) in self.open_positions.drain() {
            let price = quotes.get(&symbol)?;
            let commission = self.broker.get_trade_commission(quantity, price)?;
            self.stock_sells.push(StockSell::new(
                &symbol, quantity, price, commission, conclusion_date, execution_date));
        }

        self.order_stock_sells()?;
        self.process_trades(true)
    }

    fn merge(&mut self, mut statement: BrokerStatement) -> EmptyResult {
        if statement.period.0 != self.period.1 {
            return Err!("Non-continuous periods: {}, {}",
                formatting::format_period(self.period.0, self.period.1),
                formatting::format_period(statement.period.0, statement.period.1));
        }

        self.period.1 = statement.period.1;

        self.cash_flows.extend(statement.cash_flows.drain(..));
        self.cash_assets = statement.cash_assets;

        self.stock_buys.extend(statement.stock_buys.drain(..));
        self.stock_sells.extend(statement.stock_sells.drain(..));
        self.dividends.extend(statement.dividends.drain(..));

        self.open_positions = statement.open_positions;
        self.instrument_names.extend(statement.instrument_names.drain());

        self.validate()?;

        Ok(())
    }

    fn validate(&mut self) -> EmptyResult {
        if self.period.0 >= self.period.1 {
            return Err!("Invalid statement period: {}",
                formatting::format_period(self.period.0, self.period.1));
        }

        if self.cash_assets.is_empty() {
            return Err!("Unable to find any information about current cash assets");
        }

        self.cash_flows.sort_by_key(|cash_flow| cash_flow.date);
        self.dividends.sort_by_key(|dividend| dividend.date);

        self.order_stock_buys()?;
        self.order_stock_sells()?;

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

    fn order_stock_buys(&mut self) -> EmptyResult {
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

    fn order_stock_sells(&mut self) -> EmptyResult {
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

    fn process_trades(&mut self, emulated_sells: bool) -> EmptyResult {
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

        for stock_sell in self.stock_sells.iter_mut() {
            if stock_sell.is_processed() {
                continue;
            }

            let mut remaining_quantity = stock_sell.quantity;
            let symbol_buys = unsold_stock_buys.get_mut(&stock_sell.symbol).ok_or_else(|| format!(
                "Error while processing {} position closing: There are no open positions for it",
                stock_sell.symbol
            ))?;

            while remaining_quantity > 0 {
                let mut stock_buy = symbol_buys.pop().ok_or_else(|| format!(
                    "Error while processing {} position closing: There are no open positions for it",
                    stock_sell.symbol
                ))?;

                let available = stock_buy.quantity - stock_buy.sold;
                let sell_quantity = std::cmp::min(remaining_quantity, available);
                assert!(sell_quantity > 0);

                stock_sell.sources.push(StockSellSource {
                    quantity: sell_quantity,
                    price: stock_buy.price,
                    commission: stock_buy.commission / stock_buy.quantity * sell_quantity,

                    conclusion_date: stock_buy.conclusion_date,
                    execution_date: stock_buy.execution_date,
                });

                remaining_quantity -= sell_quantity;
                stock_buy.sold += sell_quantity;

                if stock_buy.is_sold() {
                    stock_buys.push(stock_buy);
                } else {
                    symbol_buys.push(stock_buy);
                }
            }

            if emulated_sells {
                self.cash_assets.deposit(stock_sell.price * stock_sell.quantity);
                self.cash_assets.withdraw(stock_sell.commission);
            }
        }

        for (_, mut symbol_buys) in unsold_stock_buys.drain() {
            stock_buys.extend(symbol_buys.drain(..));
        }
        drop(unsold_stock_buys);

        assert_eq!(stock_buys.len(), stock_buys_num);
        self.stock_buys = stock_buys;
        self.order_stock_buys()?;

        self.validate_open_positions()?;

        Ok(())
    }

    fn validate_open_positions(&self) -> EmptyResult {
        let mut open_positions = HashMap::new();

        for stock_buy in &self.stock_buys {
            if stock_buy.is_sold() {
                continue;
            }

            let quantity = stock_buy.quantity - stock_buy.sold;

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
    statement_dir_path: &str, statement_reader: &BrokerStatementReader
) -> GenericResult<Vec<String>> {
    let mut file_names = Vec::new();

    for entry in fs::read_dir(statement_dir_path)? {
        let file_name = entry?.file_name().into_string().map_err(|file_name| format!(
            "Got an invalid file name: {:?}", file_name.to_string_lossy()))?;

        if statement_reader.is_statement(&file_name) {
            file_names.push(file_name);
        }
    }

    Ok(file_names)
}

pub trait BrokerStatementReader {
    fn is_statement(&self, file_name: &str) -> bool;
    fn read(&self, path: &str) -> GenericResult<BrokerStatement>;
}

pub struct BrokerStatementBuilder {
    broker: BrokerInfo,
    period: Option<(Date, Date)>,

    starting_assets: Option<bool>,
    cash_flows: Vec<CashAssets>,
    cash_assets: MultiCurrencyCashAccount,

    stock_buys: Vec<StockBuy>,
    stock_sells: Vec<StockSell>,
    dividends: Vec<Dividend>,

    open_positions: HashMap<String, u32>,
    instrument_names: HashMap<String, String>,
}

impl BrokerStatementBuilder {
    fn new(broker: BrokerInfo) -> BrokerStatementBuilder {
        BrokerStatementBuilder {
            broker: broker,
            period: None,

            starting_assets: None,
            cash_flows: Vec::new(),
            cash_assets: MultiCurrencyCashAccount::new(),

            stock_buys: Vec::new(),
            stock_sells: Vec::new(),
            dividends: Vec::new(),

            open_positions: HashMap::new(),
            instrument_names: HashMap::new(),
        }
    }

    fn set_period(&mut self, period: (Date, Date)) -> EmptyResult {
        set_option("statement period", &mut self.period, period)
    }

    fn set_starting_assets(&mut self, exists: bool) -> EmptyResult {
        set_option("starting assets", &mut self.starting_assets, exists)
    }

    fn get(self) -> GenericResult<BrokerStatement> {
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

#[derive(Debug)]
pub struct StockBuy {
    pub symbol: String,
    pub quantity: u32,
    pub price: Cash,
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,

    sold: u32,
}

impl StockBuy {
    pub fn new(
        symbol: &str, quantity: u32, price: Cash, commission: Cash,
        conclusion_date: Date, execution_date: Date,
    ) -> StockBuy {
        StockBuy {
            symbol: symbol.to_owned(), quantity, price, commission,
            conclusion_date, execution_date, sold: 0,
        }
    }

    fn is_sold(&self) -> bool {
        self.sold == self.quantity
    }
}

#[derive(Debug)]
pub struct StockSell {
    pub symbol: String,
    pub quantity: u32,
    pub price: Cash,
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,

    sources: Vec<StockSellSource>,
}

impl StockSell {
    pub fn new(
        symbol: &str, quantity: u32, price: Cash, commission: Cash,
        conclusion_date: Date, execution_date: Date,
    ) -> StockSell {
        StockSell {
            symbol: symbol.to_owned(), quantity, price, commission,
            conclusion_date, execution_date, sources: Vec::new(),
        }
    }

    fn is_processed(&self) -> bool {
        !self.sources.is_empty()
    }
}

#[derive(Debug)]
pub struct StockSellSource {
    quantity: u32,
    price: Cash,
    commission: Cash,

    conclusion_date: Date,
    execution_date: Date,
}

impl StockSell {
    pub fn tax_to_pay(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Decimal> {
        // TODO: We need to use exactly the same rounding logic as is tax statement

        let mut purchase_cost = dec!(0);

        for source in &self.sources {
            purchase_cost += converter.convert_to(
                source.execution_date, source.price * source.quantity, country.currency)?;

            purchase_cost += converter.convert_to(
                source.conclusion_date, source.commission, country.currency)?;
        }

        let mut sell_revenue = converter.convert_to(
            self.execution_date, self.price * self.quantity, country.currency)?;

        sell_revenue -= converter.convert_to(
            self.conclusion_date, self.commission, country.currency)?;

        let income = sell_revenue - purchase_cost;
        if income.is_sign_negative() {
            return Ok(dec!(0));
        }

        Ok(country.tax_to_pay(income, None))
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
        let paid_tax = converter.convert_to(self.date, self.paid_tax, country.currency)?;
        Ok(country.tax_to_pay(amount, Some(paid_tax)))
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