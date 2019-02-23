use crate::broker_statement::BrokerStatement;
use crate::config::Config;
use crate::core::EmptyResult;
use crate::currency::converter::CurrencyConverter;
use crate::db;
use crate::quotes::Quotes;

use self::performance::PortfolioPerformanceAnalyser;

mod deposit_emulator;
mod performance;

pub fn analyse(config: &Config, portfolio_name: &str) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;

    let database = db::connect(&config.db_path)?;
    let converter = CurrencyConverter::new(database.clone(), false);
    let mut quotes = Quotes::new(&config, database.clone());

    let mut statement = BrokerStatement::read(config, portfolio.broker, &portfolio.statements)?;
    statement.check_date();
    statement.batch_quotes(&mut quotes);
    statement.emulate_sellout(&mut quotes)?;

    for currency in ["USD", "RUB"].iter() {
        PortfolioPerformanceAnalyser::analyse(
            &statement, &portfolio.tax_deductions, *currency, &converter)?;
    }

    Ok(())
}

pub fn simulate_sell(config: &Config, portfolio_name: &str, positions: &Vec<(u32, String)>) -> EmptyResult {
    Err!("Not implemented yet")
}