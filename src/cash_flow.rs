use chrono::Datelike;

use crate::broker_statement::BrokerStatement;
use crate::config::Config;
use crate::core::EmptyResult;
use crate::util;

// FIXME(konishchev): It's only a prototype
pub fn generate_cash_flow_report(config: &Config, portfolio_name: &str, year: Option<i32>) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let broker_statement = BrokerStatement::read(
        config, portfolio.broker, &portfolio.statements, portfolio.get_tax_remapping()?, true)?;

    if let Some(year) = year {
        if year > util::today().year() {
            return Err!("An attempt to generate cash flow report for the future");
        }

        broker_statement.check_period_against_tax_year(year)?;
    }

    Ok(())
}