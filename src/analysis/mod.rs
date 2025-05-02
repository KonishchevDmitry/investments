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

pub fn backtest(config: &Config) -> EmptyResult {
    let commission_spec = crate::brokers::plans::tbank::premium();
    let instrument = |symbol: &str| BenchmarkInstrument::new(symbol, Exchange::Moex, commission_spec.clone());

    let (sber, tbank, vtb) = ("Sber", "T-Bank", "VTB");
    let benchmark = |name: &str, provider: &str, symbol: &str| Benchmark::new(name, instrument(symbol)).with_provider(provider);

    let benchmarks = [
        benchmark("Russian stocks", sber, "FXRL")
            .then(date!(2021, 7, 29), instrument("SBMX"))?,
        benchmark("Russian stocks", tbank, "FXRL")
            .then(date!(2021, 7, 29), instrument("TMOS"))?,
        benchmark("Russian stocks", vtb, "FXRL")
            .then(date!(2021, 7, 29), instrument("VTBX"))?
            .then_rename(date!(2022, 7, 22), instrument("EQMX"))?,

        benchmark("Russian money market", sber, "FXRB")
            .then(date!(2018,  3,  7), instrument("FXMM"))?
            .then(date!(2021, 12, 30), instrument("SBMM"))?,
        benchmark("Russian money market", tbank, "FXRB")
            .then(date!(2018,  3,  7), instrument("FXMM"))?
            .then(date!(2021, 12, 30), instrument("SBMM"))?
            .then(date!(2023,  7, 14), instrument("TMON"))?,
        benchmark("Russian money market", vtb, "FXRB")
            .then(date!(2018,  3,  7), instrument("FXMM"))?
            .then(date!(2021, 12, 30), instrument("VTBM"))?
            .then_rename(date!(2022, 7, 22), instrument("LQDT"))?,

        benchmark("Russian government bonds", sber, "FXRB")
            .then(date!(2019,  1, 25), instrument("SBGB"))?,
        benchmark("Russian government bonds", tbank, "FXRB")
            .then(date!(2019,  1, 25), instrument("SBGB"))?
            .then(date!(2024, 12, 17), instrument("TOFZ"))?,

        benchmark("Russian corporate bonds", sber, "FXRB")
            .then(date!(2020,  5, 20), instrument("SBRB"))?,
        benchmark("Russian corporate bonds", tbank, "FXRB")
            .then(date!(2020,  5, 20), instrument("SBRB"))?
            .then(date!(2021,  8,  6), instrument("TBRU"))?,
        benchmark("Russian corporate bonds", vtb, "FXRB")
            .then(date!(2020,  5, 20), instrument("SBRB"))?
            .then(date!(2021,  8,  6), instrument("VTBB"))?
            .then_rename(date!(2022, 7, 22), instrument("OBLG"))?,

        benchmark("Russian corporate eurobonds", sber, "FXRU")
            .then(date!(2020,  9, 24), instrument("SBCB"))?
            .then(date!(2022,  1, 25), instrument("SBMM"))? // SBCB was frozen for this period. Ideally we need some stub only for new deposits
            .then(date!(2023, 12, 15), instrument("SBCB"))?, // The open price is equal to close price of previous SBCB interval
        benchmark("Russian corporate eurobonds", tbank, "FXRU")
            .then(date!(2020,  9, 24), instrument("SBCB"))?
            .then(date!(2022,  1, 25), instrument("SBMM"))? // SBCB was frozen for this period. Ideally we need some stub only for new deposits
            .then(date!(2023, 12, 15), instrument("SBCB"))? // The open price is equal to close price of previous SBCB interval
            .then(date!(2024,  4,  1), instrument("TLCB"))?,

        benchmark("Gold", sber, "FXRU")
            .then(date!(2018,  3,  7), instrument("FXGD"))?
            .then(date!(2020,  7, 15), instrument("VTBG"))?
            .then_rename(date!(2022, 7, 22), instrument("GOLD"))?
            .then(date!(2022, 11, 21), instrument("SBGD"))?,
        benchmark("Gold", tbank, "FXRU")
            .then(date!(2018,  3,  7), instrument("FXGD"))?
            .then(date!(2020,  7, 15), instrument("VTBG"))?
            .then_rename(date!(2022, 7, 22), instrument("GOLD"))?
            .then(date!(2024, 11,  5), instrument("TGLD"))?,
        benchmark("Gold", vtb, "FXRU")
            .then(date!(2018,  3,  7), instrument("FXGD"))?
            .then(date!(2020,  7, 15), instrument("VTBG"))?
            .then_rename(date!(2022, 7, 22), instrument("GOLD"))?,
    ];

    let (converter, quotes) = load_tools(config)?;
    let reading_strictness = ReadingStrictness::empty();
    let multiple_portfolios = config.portfolios.len() > 1;

    let mut statements = Vec::new();

    for portfolio in &config.portfolios {
        // FIXME(konishchev): Drop it
        if portfolio.name != "tbank-iia" {
            continue;
        }
        let _logging_context = multiple_portfolios.then(|| GlobalContext::new(&portfolio.name));
        statements.push(load_portfolio(config, portfolio, reading_strictness)?);
    }

    // FIXME(konishchev): Multiple currencies support
    // FIXME(konishchev): Check analysis virtual performance calculation logic
    backtesting::backtest("RUB", &benchmarks, &statements, converter, quotes)
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
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_deref())?;
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