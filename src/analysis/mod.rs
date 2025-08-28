pub mod backtesting;
pub mod deposit;
mod inflation;
pub mod performance;
mod sell_simulation;

use std::collections::HashMap;
use std::rc::Rc;

use easy_logging::GlobalContext;
use itertools::Itertools;

use crate::broker_statement::{BrokerStatement, ReadingStrictness};
use crate::config::{Config, PortfolioConfig};
use crate::core::GenericResult;
use crate::currency::converter::{CurrencyConverter, CurrencyConverterRc};
use crate::db;
use crate::metrics::backfilling;
use crate::quotes::{Quotes, QuotesRc};
use crate::taxes::{LtoDeductionCalculator, TaxCalculator};
use crate::telemetry::TelemetryRecordBuilder;
use crate::types::Decimal;

use self::backtesting::{BenchmarkBacktestingResult, BenchmarkPerformanceType};
use self::performance::PortfolioAnalyser;
use self::performance::config::{AssetGroupConfig, PerformanceMergingConfig};
use self::performance::statistics::PortfolioStatistics;

pub fn analyse(
    config: &Config, portfolio_name: Option<&str>, include_closed_positions: bool,
    asset_groups: &HashMap<String, AssetGroupConfig>, merge_performance: Option<&PerformanceMergingConfig>,
    interactive: bool,
) -> GenericResult<(PortfolioStatistics, QuotesRc, TelemetryRecordBuilder)> {
    let mut telemetry = TelemetryRecordBuilder::new();

    let country = config.get_tax_country();
    let (converter, quotes) = load_tools(config)?;
    let reading_strictness = ReadingStrictness::REPO_TRADES | ReadingStrictness::TAX_EXEMPTIONS;

    let portfolios = load_portfolios(config, portfolio_name, reading_strictness)?;
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

    Ok((statistics.commit(), quotes, telemetry))
}

pub fn backtest(
    config: &Config, portfolio_name: Option<&str>, backfill: bool, interactive: Option<BenchmarkPerformanceType>,
) -> GenericResult<(Vec<BenchmarkBacktestingResult>, TelemetryRecordBuilder)> {
    let mut telemetry = TelemetryRecordBuilder::new();
    let (converter, quotes) = load_tools(config)?;

    let backfill = backfill.then(|| config.metrics.backfilling.as_ref().ok_or(
        "Metrics backfilling settings aren't specified in the configuration file")).transpose()?;

    let statements = load_portfolios(config, portfolio_name, ReadingStrictness::empty())?
        .into_iter().map(|(portfolio, statement)| {
            telemetry.add_broker(portfolio.broker);
            statement
        })
        .collect_vec();

    let (results, metrics) = backtesting::backtest(
        &config.backtesting, &statements, converter.clone(), quotes.clone(),
        backfill.map(|config| config.min_performance_period), interactive)?;

    if let Some(config) = backfill {
        backfilling::backfill(config, metrics).map_err(|e| format!(
            "Failed to backfill backtesting results: {e}"))?;
    }

    Ok((results, telemetry))
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

fn load_portfolios<'a>(
    config: &'a Config, name: Option<&str>, reading_strictness: ReadingStrictness,
) -> GenericResult<Vec<(&'a PortfolioConfig, BrokerStatement)>> {
    let mut portfolios = Vec::new();

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
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_deref())?;
    BrokerStatement::read(
        broker, portfolio.statements_path()?, &portfolio.get_symbol_remapping()?, &portfolio.instrument_internal_ids,
        &portfolio.instrument_names, portfolio.get_tax_remapping()?, &portfolio.tax_exemptions,
        &portfolio.corporate_actions, strictness)
}

fn load_tools(config: &Config) -> GenericResult<(CurrencyConverterRc, QuotesRc)> {
    let database = db::connect(&config.db_path)?;
    let quotes = Rc::new(Quotes::new(config, database.clone())?);
    let converter = CurrencyConverter::new(database, Some(quotes.clone()), false);
    Ok((converter, quotes))
}