use std::rc::Rc;

use crate::broker_statement::BrokerStatement;
use crate::commissions::CommissionCalc;
use crate::config::{Config, PortfolioConfig};
use crate::core::{GenericResult, EmptyResult};
use crate::currency::converter::CurrencyConverter;
use crate::db;
use crate::quotes::Quotes;

use self::performance::PortfolioPerformanceAnalyser;

pub mod deposit_emulator;
mod performance;
mod sell_simulation;

pub fn analyse(config: &Config, portfolio_name: &str, show_closed_positions: bool) -> EmptyResult {
    let (portfolio, mut statement, converter, quotes) = load(config, portfolio_name, false)?;
    let mut commission_calc = CommissionCalc::new(statement.broker.commission_spec.clone());

    statement.check_date();
    statement.batch_quotes(&quotes);

    for (symbol, quantity) in statement.open_positions.clone() {
        statement.emulate_sell(&symbol, quantity, quotes.get(&symbol)?, &mut commission_calc)?;
    }
    statement.process_trades()?;
    statement.emulate_commissions(commission_calc);
    statement.merge_symbols(&portfolio.merge_performance).map_err(|e| format!(
        "Invalid performance merging configuration: {}", e))?;

    for currency in ["USD", "RUB"].iter() {
        PortfolioPerformanceAnalyser::analyse(
            &statement, &portfolio, *currency, &converter, show_closed_positions)?;
    }

    Ok(())
}

pub fn simulate_sell(config: &Config, portfolio_name: &str, positions: &[(String, Option<u32>)]) -> EmptyResult {
    let (portfolio, statement, converter, quotes) = load(config, portfolio_name, true)?;
    sell_simulation::simulate_sell(portfolio, statement, &converter, &quotes, positions)
}

fn load<'a>(config: &'a Config, portfolio_name: &str, strict_mode: bool) -> GenericResult<
    (&'a PortfolioConfig, BrokerStatement, CurrencyConverter, Rc<Quotes>)
> {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_ref())?;

    let statement = BrokerStatement::read(
        broker, &portfolio.statements, portfolio.get_tax_remapping()?, strict_mode)?;

    let database = db::connect(&config.db_path)?;
    let quotes = Rc::new(Quotes::new(&config, database.clone())?);
    let converter = CurrencyConverter::new(database, Some(quotes.clone()), false);

    Ok((portfolio, statement, converter, quotes))
}