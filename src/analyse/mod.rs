use crate::broker_statement::BrokerStatement;
use crate::config::{Config, PortfolioConfig};
use crate::core::{GenericResult, EmptyResult};
use crate::currency::converter::CurrencyConverter;
use crate::db;
use crate::quotes::Quotes;

use self::performance::PortfolioPerformanceAnalyser;

mod deposit_emulator;
mod performance;
mod sell_simulation;

pub fn analyse(config: &Config, portfolio_name: &str) -> EmptyResult {
    let (portfolio, mut statement, converter, mut quotes) = load(config, portfolio_name)?;

    statement.check_date();
    statement.batch_quotes(&mut quotes);

    for (symbol, &quantity) in statement.open_positions.clone().iter() {
        statement.emulate_sell(&symbol, quantity, quotes.get(&symbol)?)?;
    }
    statement.process_trades()?;

    for currency in ["USD", "RUB"].iter() {
        PortfolioPerformanceAnalyser::analyse(
            &statement, &portfolio.tax_deductions, *currency, &converter)?;
    }

    Ok(())
}

pub fn simulate_sell(config: &Config, portfolio_name: &str, positions: &[(String, Option<u32>)]) -> EmptyResult {
    let (_portfolio, statement, converter, quotes) = load(config, portfolio_name)?;
    sell_simulation::simulate_sell(statement, &converter, quotes, positions)
}

fn load<'a>(config: &'a Config, portfolio_name: &str) -> GenericResult<
    (&'a PortfolioConfig, BrokerStatement, CurrencyConverter, Quotes)
> {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let statement = BrokerStatement::read(config, portfolio.broker, &portfolio.statements)?;

    let database = db::connect(&config.db_path)?;
    let converter = CurrencyConverter::new(database.clone(), false);
    let quotes = Quotes::new(&config, database.clone());

    Ok((portfolio, statement, converter, quotes))
}