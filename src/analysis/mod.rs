mod backtesting;
pub mod config;
pub mod deposit_emulator;
mod deposit_performance;
mod inflation;
mod instrument_view;
mod portfolio_analysis;
mod portfolio_performance_types;
mod portfolio_performance;
mod sell_simulation;
pub mod portfolio_statistics;

use std::collections::HashMap;
use std::rc::Rc;

use easy_logging::GlobalContext;

use crate::broker_statement::{BrokerStatement, ReadingStrictness};
use crate::config::{Config, PortfolioConfig};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::converter::{CurrencyConverter, CurrencyConverterRc};
use crate::currency::Cash;
use crate::db;
use crate::exchanges::Exchange;
use crate::quotes::{Quotes, QuotesRc};
use crate::taxes::{LtoDeductionCalculator, TaxCalculator};
use crate::telemetry::TelemetryRecordBuilder;
use crate::types::Decimal;

use self::backtesting::{Benchmark, BenchmarkInstrument};
use self::config::{AssetGroupConfig, PerformanceMergingConfig};
use self::portfolio_analysis::PortfolioAnalyser;
use self::portfolio_statistics::PortfolioStatistics;

pub use self::portfolio_performance_types::PerformanceAnalysisMethod;

pub fn analyse(
    config: &Config, portfolio_name: Option<&str>, include_closed_positions: bool,
    asset_groups: &HashMap<String, AssetGroupConfig>, merge_performance: Option<&PerformanceMergingConfig>,
    interactive: bool,
) -> GenericResult<(PortfolioStatistics, QuotesRc, TelemetryRecordBuilder)> {
    let mut telemetry = TelemetryRecordBuilder::new();

    let country = config.get_tax_country();
    let (converter, quotes) = load_tools(config)?;

    let portfolios = load_portfolios(config, portfolio_name)?;
    for (portfolio, _statement) in &portfolios {
        telemetry.add_broker(portfolio.broker);
    }

    let mut statistics = PortfolioStatistics::new(country.clone());

    let analyser = PortfolioAnalyser {
        country: country.clone(),
        interactive, include_closed_positions,

        asset_groups, merge_performance,
        quotes: quotes.clone(), converter,

        lto_calc: LtoDeductionCalculator::new(),
        taxes: TaxCalculator::new(country),
    };
    analyser.process(portfolios, &mut statistics)?;

    Ok((statistics, quotes, telemetry))
}

pub fn simulate_sell(
    config: &Config, portfolio_name: &str, positions: Option<Vec<(String, Option<Decimal>)>>,
    base_currency: Option<&str>,
) -> GenericResult<TelemetryRecordBuilder> {
    let portfolio = config.get_portfolio(portfolio_name)?;

    let statement = load_portfolio(config, portfolio,
        ReadingStrictness::TRADE_SETTLE_DATE | ReadingStrictness::OTC_INSTRUMENTS | ReadingStrictness::TAX_EXEMPTIONS)?;
    let (converter, quotes) = load_tools(config)?;

    sell_simulation::simulate_sell(
        &config.get_tax_country(), portfolio, statement,
        converter, &quotes, positions, base_currency)?;

    Ok(TelemetryRecordBuilder::new_with_broker(portfolio.broker))
}

// FIXME(konishchev): A work in progress mockup
pub fn backtest(config: &Config) -> EmptyResult {
    let (converter, quotes) = load_tools(config)?;

    let benchmarks = vec![
        Benchmark::new("Russian stocks", BenchmarkInstrument::new("SBMX", Exchange::Moex)),
    ];

    let currency = "RUB";

    let portfolios = load_portfolios(config, Some("sber-iia"))?;

    let mut cash_flows = Vec::new();
    let mut net_value = Cash::zero(currency);

    for (_portfolio, statement) in portfolios {
        net_value += statement.net_value(&converter, &quotes, currency, true)?;
        cash_flows.extend(statement.deposits_and_withdrawals);
    }

    cash_flows.sort_by_key(|cash_flow| cash_flow.date);

    println!("Portfoio: {net_value}");

    for benchmark in &benchmarks {
        let result = benchmark.backtest(currency, &cash_flows, &converter, &quotes)?;
        println!("{}: {result}", benchmark.name);
    }

    Ok(())
}

fn load_portfolios<'a>(config: &'a Config, name: Option<&str>) -> GenericResult<Vec<(&'a PortfolioConfig, BrokerStatement)>> {
    let mut portfolios = Vec::new();
    let reading_strictness = ReadingStrictness::REPO_TRADES | ReadingStrictness::TAX_EXEMPTIONS;

    if let Some(name) = name {
        let portfolio = config.get_portfolio(name)?;
        let statement = load_portfolio(config, portfolio, reading_strictness)?;
        portfolios.push((portfolio, statement));
    } else {
        if config.portfolios.is_empty() {
            return Err!("There is no any portfolio defined in the configuration file")
        }

        let multiple = config.portfolios.len() > 1;

        for portfolio in &config.portfolios {
            let _logging_context = multiple.then(|| GlobalContext::new(&portfolio.name));
            let statement = load_portfolio(config, portfolio, reading_strictness)?;
            portfolios.push((portfolio, statement));
        }
    }

    Ok(portfolios)
}

fn load_portfolio(config: &Config, portfolio: &PortfolioConfig, strictness: ReadingStrictness) -> GenericResult<BrokerStatement> {
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_ref())?;
    BrokerStatement::read(
        broker, portfolio.statements_path()?, &portfolio.symbol_remapping, &portfolio.instrument_internal_ids,
        &portfolio.instrument_names, portfolio.get_tax_remapping()?, &portfolio.tax_exemptions,
        &portfolio.corporate_actions, strictness)
}

fn load_tools(config: &Config) -> GenericResult<(CurrencyConverterRc, QuotesRc)> {
    let database = db::connect(&config.db_path)?;
    let quotes = Rc::new(Quotes::new(config, database.clone())?);
    let converter = CurrencyConverter::new(database, Some(quotes.clone()), false);
    Ok((converter, quotes))
}