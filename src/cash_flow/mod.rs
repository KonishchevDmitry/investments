mod types;

use chrono::Datelike;

use crate::broker_statement::BrokerStatement;
use crate::config::Config;
use crate::core::EmptyResult;
use crate::currency::MultiCurrencyCashAccount;
use crate::util;

use types::CashFlow;

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

    let mut cash_assets = MultiCurrencyCashAccount::new();
    let mut cash_flows: Vec<Box<dyn CashFlow>> = Vec::new();

    for assets in broker_statement.cash_flows {
        cash_flows.push(Box::new(assets));
    }

    cash_flows.sort_by_key(|cash_flow| cash_flow.date());
    for cash_flow in &cash_flows {
        cash_assets.deposit(cash_flow.amount());
        println!("{}: {} - {}", cash_flow.date(), cash_flow.description(), cash_flow.amount());
    }

    for assets in cash_assets.iter() {
        println!("{}", assets);
    }

    for assets in broker_statement.cash_assets.iter() {
        println!("{}", assets);
    }

    Ok(())
}