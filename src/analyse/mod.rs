use std::rc::Rc;

use crate::broker_statement::BrokerStatement;
use crate::commissions::CommissionCalc;
use crate::config::{Config, PortfolioConfig};
use crate::core::{GenericResult, EmptyResult};
use crate::currency::converter::CurrencyConverter;
use crate::db;
use crate::localities;
use crate::quotes::Quotes;

use self::performance::PortfolioPerformanceAnalyser;

pub mod deposit_emulator;
mod performance;
mod sell_simulation;

pub fn analyse(config: &Config, portfolio_name: &str, show_closed_positions: bool) -> EmptyResult {
    let mut portfolios = Vec::new();

    if portfolio_name == "all" {
        if config.portfolios.is_empty() {
            return Err!("There is no any portfolio defined in the configuration file")
        }

        for portfolio in &config.portfolios {
            let statement = load_portfolio(config, portfolio, false)?;
            portfolios.push((portfolio, statement));
        }
    } else {
        let portfolio = config.get_portfolio(portfolio_name)?;
        let statement = load_portfolio(config, portfolio, false)?;
        portfolios.push((portfolio, statement));
    }

    let country = localities::russia();
    let (converter, quotes) = load_tools(config)?;

    for (_, statement) in &mut portfolios {
        statement.batch_quotes(&quotes);
    }

    for (portfolio, statement) in &mut portfolios {
        statement.check_date();

        let mut commission_calc = CommissionCalc::new(statement.broker.commission_spec.clone());

        for (symbol, quantity) in statement.open_positions.clone() {
            statement.emulate_sell(&symbol, quantity, quotes.get(&symbol)?, &mut commission_calc)?;
        }
        statement.process_trades()?;
        statement.emulate_commissions(commission_calc);

        statement.merge_symbols(&portfolio.merge_performance).map_err(|e| format!(
            "Invalid performance merging configuration: {}", e))?;
    }

    for &currency in &["USD", "RUB"] {
        let mut analyser = PortfolioPerformanceAnalyser::new(
            country, currency, &converter, show_closed_positions);

        for (portfolio, statement) in &mut portfolios {
            analyser.add(&portfolio, &statement)?;
        }

        analyser.analyse()?;
    }

    Ok(())
}

pub fn simulate_sell(config: &Config, portfolio_name: &str, positions: &[(String, Option<u32>)]) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let statement = load_portfolio(config, portfolio, true)?;
    let (converter, quotes) = load_tools(config)?;
    sell_simulation::simulate_sell(portfolio, statement, &converter, &quotes, positions)
}

fn load_portfolio(config: &Config, portfolio: &PortfolioConfig, strict_mode: bool) -> GenericResult<BrokerStatement> {
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_ref())?;
    BrokerStatement::read(
        broker, &portfolio.statements, &portfolio.symbol_remapping, &portfolio.instrument_names,
        portfolio.get_tax_remapping()?, strict_mode)
}

fn load_tools(config: &Config) -> GenericResult<(CurrencyConverter, Rc<Quotes>)> {
    let database = db::connect(&config.db_path)?;
    let quotes = Rc::new(Quotes::new(&config, database.clone())?);
    let converter = CurrencyConverter::new(database, Some(quotes.clone()), false);
    Ok((converter, quotes))
}