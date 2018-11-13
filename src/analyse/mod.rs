use broker_statement::BrokerStatement;
use config::Config;
use core::EmptyResult;
use currency::converter::CurrencyConverter;
use db;
use quotes::Quotes;

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