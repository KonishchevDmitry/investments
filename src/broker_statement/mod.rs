use std::collections::HashMap;

use chrono::Duration;

use core::{EmptyResult, GenericResult};
use config::BrokerConfig;
use currency::{self, Cash, CashAssets};
use currency::converter::CurrencyConverter;
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
    pub dividends: Vec<Dividend>,
    pub instrument_names: HashMap<String, String>,
    pub total_value: Cash,
}

impl BrokerStatement {
    pub fn format_period(&self) -> String {
        format!("{} - {}",
                util::format_date(self.period.0),
                util::format_date(self.period.1 - Duration::days(1)))
    }

    pub fn get_instrument_name(&self, ticker: &str) -> GenericResult<String> {
        let name = self.instrument_names.get(ticker).ok_or_else(|| format!(
            "Unable to find {:?} instrument name in the broker statement", ticker))?;
        Ok(format!("{} ({})", name, ticker))
    }
}

struct BrokerStatementBuilder {
    broker: BrokerInfo,
    allow_partial: bool,

    period: Option<(Date, Date)>,
    starting_value: Option<Cash>,
    deposits: Vec<CashAssets>,
    dividends: Vec<Dividend>,
    open_positions: HashMap<String, u32>,
    instrument_names: HashMap<String, String>,
    total_value: Option<Cash>,
}

impl BrokerStatementBuilder {
    fn new(broker: BrokerInfo, allow_partial: bool) -> BrokerStatementBuilder {
        BrokerStatementBuilder {
            broker: broker,
            allow_partial: allow_partial,

            period: None,
            starting_value: None,
            deposits: Vec::new(),
            dividends: Vec::new(),
            open_positions: HashMap::new(),
            instrument_names: HashMap::new(),
            total_value: None,
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

        for deposit in &self.deposits {
            if deposit.date < min_date || deposit.date > max_date {
                return Err!("Got a deposit outside of statement period: {}",
                    util::format_date(deposit.date));
            }
        }

        self.deposits.sort_by_key(|deposit| deposit.date);
        self.dividends.sort_by_key(|dividend| dividend.date);

        let statement = BrokerStatement {
            broker: self.broker,
            period: period,
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
