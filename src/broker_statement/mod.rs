use std;
use std::collections::{HashMap, HashSet};

use chrono::Duration;

use core::{EmptyResult, GenericResult};
use config::BrokerConfig;
use currency::{self, Cash, CashAssets, MultiCurrencyCashAccount};
use currency::converter::CurrencyConverter;
use quotes::Quotes;
use regulations::Country;
use types::{Date, Decimal};
use util;

pub mod ib;

// TODO: Take care of stock splitting
#[derive(Debug)]
pub struct BrokerStatement {
    pub broker: BrokerInfo,
    pub period: (Date, Date),

    pub deposits: Vec<CashAssets>,
    pub cash_assets: MultiCurrencyCashAccount,

    stock_buys: Vec<StockBuy>,
    pub stock_sells: Vec<StockSell>,
    pub dividends: Vec<Dividend>,

    pub open_positions: HashMap<String, u32>,
    instrument_names: HashMap<String, String>,
}

impl BrokerStatement {
    pub fn format_period(&self) -> String {
        format!("{} - {}",
                util::format_date(self.period.0),
                util::format_date(self.period.1 - Duration::days(1)))
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
                commission: Cash::new("USD", dec!(1)),  // FIXME: Get from broker info
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

struct BrokerStatementBuilder {
    broker: BrokerInfo,
    allow_partial: bool,

    period: Option<(Date, Date)>,

    starting_value: Option<Cash>,
    deposits: Vec<CashAssets>,
    cash_assets: MultiCurrencyCashAccount,

    stock_buys: Vec<StockBuy>,
    stock_sells: Vec<StockSell>,
    dividends: Vec<Dividend>,

    open_positions: HashMap<String, u32>,
    instrument_names: HashMap<String, String>,
}

impl BrokerStatementBuilder {
    fn new(broker: BrokerInfo, allow_partial: bool) -> BrokerStatementBuilder {
        BrokerStatementBuilder {
            broker: broker,
            allow_partial: allow_partial,

            period: None,

            starting_value: None,
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

    fn set_starting_value(&mut self, starting_value: Cash) -> EmptyResult {
        set_option("starting value", &mut self.starting_value, starting_value)
    }

    fn get(mut self) -> GenericResult<BrokerStatement> {
        let period = get_option("statement period", self.period)?;

        let min_date = period.0;
        let max_date = period.1 - Duration::days(1);

        let starting_value = get_option("starting value", self.starting_value)?;
        if !self.allow_partial && !starting_value.is_zero() {
            return Err!(
                "Invalid broker statement period: it must start before any activity on the account");
        }

        if self.cash_assets.is_empty() {
            return Err!("Unable to find any information about current cash assets");
        }

        self.deposits.sort_by_key(|deposit| deposit.date);
        self.stock_buys.sort_by_key(|transaction| transaction.date);
        self.stock_sells.sort_by_key(|transaction| transaction.date);
        self.dividends.sort_by_key(|dividend| dividend.date);

        let mut open_positions = HashMap::new();

        for stock_buy in &self.stock_buys {
            if let Some(position) = open_positions.get_mut(&stock_buy.symbol) {
                *position += stock_buy.quantity;
                continue;
            }

            open_positions.insert(stock_buy.symbol.clone(), stock_buy.quantity);
        }

        if open_positions != self.open_positions {
            return Err!("The calculated open positions don't match the specified in the statement");
        }

        for deposit in &self.deposits {
            if deposit.date < min_date || deposit.date > max_date {
                return Err!("Got a deposit outside of statement period: {}",
                    util::format_date(deposit.date));
            }
        }

        let statement = BrokerStatement {
            broker: self.broker,
            period: period,

            deposits: self.deposits,
            cash_assets: self.cash_assets,

            stock_buys: self.stock_buys,
            stock_sells: self.stock_sells,
            dividends: self.dividends,

            open_positions: self.open_positions,
            instrument_names: self.instrument_names,
        };

        debug!("{:#?}", statement);
        return Ok(statement)
    }
}

#[derive(Debug)]
pub struct BrokerInfo {
    pub name: &'static str,
    config: BrokerConfig,
}

impl BrokerInfo {
    pub fn get_deposit_commission(&self, assets: CashAssets) -> GenericResult<Decimal> {
        let currency = assets.cash.currency;

        let commission_spec = match self.config.deposit_commissions.get(currency) {
            Some(commission_spec) => commission_spec,
            None => return Err!(concat!(
                "Unable to calculate commission for {} deposit to {}: there is no commission ",
                "specification in the configuration file"), currency, self.name),
        };

        Ok(commission_spec.fixed_amount)
    }
}

#[derive(Debug)]
pub struct StockBuy {
    date: Date,
    symbol: String,
    quantity: u32,
    price: Cash,
    commission: Cash,

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