use std::{self, fs};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::Duration;

use brokers::BrokerInfo;
use config::{Config, Broker};
use core::{EmptyResult, GenericResult};
use currency::{self, Cash, CashAssets, MultiCurrencyCashAccount};
use currency::converter::CurrencyConverter;
use formatting;
use quotes::Quotes;
use regulations::Country;
use types::{Date, Decimal};
use util;

mod ib;
mod open_broker;

// TODO: Take care of stock splitting
#[derive(Debug)]
pub struct BrokerStatement {
    pub broker: BrokerInfo,
    pub period: (Date, Date),

    starting_assets: bool,
    pub deposits: Vec<CashAssets>,
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
            Broker::InteractiveBrokers => ib::StatementReader::new(
                config.brokers.interactive_brokers.as_ref().ok_or_else(|| format!(
                    "Interactive Brokers configuration is not set in the configuration file"))?),

            Broker::OpenBroker => open_broker::StatementReader::new(
                config.brokers.open_broker.as_ref().ok_or_else(|| format!(
                    "Open Broker configuration is not set in the configuration file"))?),
        };

        let mut file_names = get_statement_files(statement_dir_path, &statement_reader).map_err(|e| format!(
            "Error while reading {:?}: {}", statement_dir_path, e))?;
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

        debug!("{:#?}", joint_statement);
        Ok(joint_statement)
    }

    fn merge(&mut self, mut statement: BrokerStatement) -> EmptyResult {
        if statement.period.0 != self.period.1 {
            return Err!("Non-continuous periods: {}, {}",
                formatting::format_period(self.period.0, self.period.1),
                formatting::format_period(statement.period.0, statement.period.1));
        }

        self.period.1 = statement.period.1;

        self.deposits.extend(statement.deposits.drain(..));
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

        self.deposits.sort_by_key(|deposit| deposit.date);
        self.stock_buys.sort_by_key(|transaction| transaction.date);
        self.stock_sells.sort_by_key(|transaction| transaction.date);
        self.dividends.sort_by_key(|dividend| dividend.date);

        if !self.starting_assets {
            let mut open_positions = HashMap::new();

            for stock_buy in &self.stock_buys {
                if let Some(position) = open_positions.get_mut(&stock_buy.symbol) {
                    *position += stock_buy.quantity;
                    continue;
                }

                open_positions.insert(stock_buy.symbol.clone(), stock_buy.quantity);
            }

            if open_positions != self.open_positions {
                return Err!("The calculated open positions don't match declared ones in the statement");
            }
        }

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

        if !self.deposits.is_empty() {
            let first_date = self.deposits.first().unwrap().date;
            let last_date = self.deposits.last().unwrap().date;
            validate_date("deposit", first_date, last_date)?;
        }

        if !self.stock_buys.is_empty() {
            let first_date = self.stock_buys.first().unwrap().date;
            let last_date = self.stock_buys.last().unwrap().date;
            validate_date("stock buy", first_date, last_date)?;
        }

        if !self.stock_sells.is_empty() {
            let first_date = self.stock_sells.first().unwrap().date;
            let last_date = self.stock_sells.last().unwrap().date;
            validate_date("stock sell", first_date, last_date)?;
        }

        if !self.dividends.is_empty() {
            let first_date = self.dividends.first().unwrap().date;
            let last_date = self.dividends.last().unwrap().date;
            validate_date("dividend", first_date, last_date)?;
        }

        Ok(())
    }

    pub fn get_instrument_name(&self, symbol: &str) -> GenericResult<String> {
        let name = self.instrument_names.get(symbol).ok_or_else(|| format!(
            "Unable to find {:?} instrument name in the broker statement", symbol))?;
        Ok(format!("{} ({})", name, symbol))
    }

    pub fn batch_quotes(&self, quotes: &mut Quotes) {
        for (symbol, _) in &self.instrument_names {
            quotes.batch(&symbol);
        }
    }

    pub fn emulate_sellout(&mut self, quotes: &mut Quotes) -> EmptyResult {
        let today = util::today();
        let mut unsold_stocks = HashSet::new();

        for (symbol, mut quantity) in self.open_positions.drain() {
            let price = quotes.get(&symbol)?;
            let mut stock_sell = StockSell {
                date: today,
                symbol: symbol,
                quantity: quantity,
                price: price,
                commission: self.broker.get_trade_commission(quantity, price)?,
                sources: Vec::new(),
            };

            for stock_buy in self.stock_buys.iter_mut() {
                if stock_buy.symbol != stock_sell.symbol {
                    continue;
                }

                let available = stock_buy.quantity - stock_buy.sold;
                if available <= 0 {
                    continue;
                }

                let sell_quantity = std::cmp::min(quantity, available);

                stock_sell.sources.push(StockSellSource {
                    date: stock_buy.date,
                    quantity: sell_quantity,
                    price: stock_buy.price,
                    commission: stock_buy.commission / stock_buy.quantity * sell_quantity,
                });

                quantity -= sell_quantity;
                stock_buy.sold += sell_quantity;

                if quantity <= 0 {
                    break;
                }
            }

            if quantity > 0 {
                unsold_stocks.insert(stock_sell.symbol);
                continue;
            }

            self.cash_assets.deposit(stock_sell.price * stock_sell.quantity);
            self.cash_assets.withdraw(stock_sell.commission);
            self.stock_sells.push(stock_sell);
        }

        if !unsold_stocks.is_empty() {
            return Err!(
                "Failed to emulate sellout: Unable to sell {}: Not enough buy transactions.",
                unsold_stocks.drain().map(|symbol| symbol.to_owned()).collect::<Vec<_>>().join(", "));
        }

        for stock_buy in &self.stock_buys {
            if stock_buy.sold != stock_buy.quantity {
                unsold_stocks.insert(stock_buy.symbol.clone());
            }
        }

        if !unsold_stocks.is_empty() {
            return Err!("Failed to emulate sellout: The following stocks remain unsold: {}.",
                unsold_stocks.drain().map(|symbol| symbol.to_owned()).collect::<Vec<_>>().join(", "));
        }

        Ok(())
    }
}

fn get_statement_files(
    statement_dir_path: &str, statement_reader: &Box<BrokerStatementReader>
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
    deposits: Vec<CashAssets>,
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
            deposits: Vec::new(),
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
            deposits: self.deposits,
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
    pub date: Date,
    pub symbol: String,
    pub quantity: u32,
    pub price: Cash,
    pub commission: Cash,

    sold: u32,
}

#[derive(Debug)]
pub struct StockSell {
    pub date: Date,
    pub symbol: String,
    pub quantity: u32,
    pub price: Cash,
    pub commission: Cash,
    sources: Vec<StockSellSource>,
}

#[derive(Debug)]
pub struct StockSellSource {
    date: Date,
    quantity: u32,
    price: Cash,
    commission: Cash,
}

impl StockSell {
    pub fn tax_to_pay(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Decimal> {
        let mut purchase_cost = dec!(0);

        for source in &self.sources {
            purchase_cost += converter.convert_to(
                source.date, source.price * source.quantity, country.currency)?;

            purchase_cost += converter.convert_to(
                source.date, source.commission, country.currency)?;
        }

        let mut sell_revenue = converter.convert_to(
            self.date, self.price * self.quantity, country.currency)?;

        sell_revenue -= converter.convert_to(
            self.date, self.commission, country.currency)?;

        let income = sell_revenue - purchase_cost;

        // TODO: Declare loss?
        if income.is_sign_negative() {
            return Ok(dec!(0));
        }

        Ok(currency::round(income * country.tax_rate))
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