pub mod backfilling;
pub mod config;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{BufWriter, Write};
use std::fs::{self, File};
use std::path::Path;

use lazy_static::lazy_static;
use num_traits::ToPrimitive;
use prometheus::{self, TextEncoder, Encoder, Gauge, GaugeVec, register_gauge, register_gauge_vec};
use strum::IntoEnumIterator;

use crate::analysis;
use crate::analysis::backtesting::BenchmarkBacktestingResult;
use crate::analysis::performance::types::PerformanceAnalysisMethod;
use crate::analysis::performance::statistics::{Asset, AssetGroup, PortfolioCurrencyStatistics, LtoStatistics};
use crate::config::Config;
use crate::core::{EmptyResult, GenericError, GenericResult};
use crate::forex;
use crate::quotes::{QuoteQuery, QuotesRc};
use crate::telemetry::TelemetryRecordBuilder;
use crate::time;
use crate::types::Decimal;
use crate::util;

lazy_static! {
    static ref UPDATE_TIME: Gauge = register_simple_metric(
        "time", "Metrics generation time");

    static ref BROKERS: GaugeVec = register_metric(
        "brokers", "Net asset value by broker", &[CURRENCY_LABEL, "broker", "country"]);

    static ref ASSETS: GaugeVec = register_instrument_metric(
        "assets", "Open positions value");

    static ref NET_ASSETS: GaugeVec = register_instrument_metric(
        "net_assets", "Open positions net value");

    static ref ASSET_GROUPS: GaugeVec = register_metric(
        "asset_groups", "Net asset value of custom groups", &["name", CURRENCY_LABEL]);

    static ref PERFORMANCE: GaugeVec = register_performance_metric(
        "performance", "Instrument performance");

    static ref INCOME_STRUCTURE: GaugeVec = register_structure_metric(
        "income_structure", "Net income structure");

    static ref EXPENCES_STRUCTURE: GaugeVec = register_structure_metric(
        "expenses_structure", "Expenses structure");

    static ref PROFIT: GaugeVec = register_portfolio_metric(
        "profit", "Profit");

    static ref NET_PROFIT: GaugeVec = register_portfolio_metric(
        "net_profit", "Net profit");

    static ref PROJECTED_TAXES: GaugeVec = register_portfolio_metric(
        "projected_taxes", "Projected taxes to pay");

    static ref PROJECTED_TAX_DEDUCTIONS: GaugeVec = register_portfolio_metric(
        "projected_tax_deductions", "Projected tax deductions");

    static ref PROJECTED_COMMISSIONS: GaugeVec = register_portfolio_metric(
        "projected_commissions", "Projected commissions to pay");

    static ref LTO: GaugeVec = register_metric(
        "lto", "Long-term ownership tax exemption applying results", &["year", TYPE_LABEL]);

    static ref PROJECTED_LTO: GaugeVec = register_metric(
        "projected_lto", "Long-term ownership tax exemption projected results", &[TYPE_LABEL]);

    static ref BACKTESTING_NET_VALUE: GaugeVec = register_metric(
        BACKTESTING_NET_VALUE_NAME, "Benchmark backtesting result: net value", &[INSTRUMENT_LABEL, PROVIDER_LABEL, CURRENCY_LABEL]);

    static ref BACKTESTING_PERFORMANCE: GaugeVec = register_metric(
        BACKTESTING_PERFORMANCE_NAME, "Benchmark backtesting result: performance", &[INSTRUMENT_LABEL, PROVIDER_LABEL, TYPE_LABEL, CURRENCY_LABEL]);

    static ref FOREX_PAIRS: GaugeVec = register_metric(
        "forex_pairs", "Forex quotes", &["base", "quote"]);
}

pub fn collect(config: &Config, path: &Path) -> GenericResult<TelemetryRecordBuilder> {
    let (statistics, quotes, telemetry) = analysis::analyse(
        config, None, false, &config.metrics.asset_groups,
        Some(&config.metrics.merge_performance), false)?;

    // FIXME(konishchev): Add command line option?
    let backtesting = analysis::backtest(config, None, !config.backtesting.benchmarks.is_empty(), None, None)?;

    UPDATE_TIME.set(cast::f64(time::timestamp()));
    for statistics in &statistics.currencies {
        collect_portfolio(statistics);
    }

    collect_forex_quotes(quotes, &config.metrics.currency_rates)?;
    collect_asset_groups(&statistics.asset_groups);
    collect_backtesting(&backtesting);
    collect_lto(statistics.lto.as_ref().unwrap());

    save(path)?;

    Ok(telemetry)
}

fn collect_portfolio(statistics: &PortfolioCurrencyStatistics) {
    let currency = &statistics.currency;
    let income_structure = &statistics.real_performance.as_ref().unwrap().income_structure;

    for (broker, &value) in &statistics.brokers {
        set_metric(&BROKERS, &[currency, broker.brief_name(), broker.jurisdiction().traits().name], value);
    }

    for (instrument, portfolios) in &statistics.assets {
        let mut total = Asset::default();

        for (portfolio, asset) in portfolios {
            set_instrument_metric(&ASSETS, portfolio, currency, instrument, asset.value);
            set_instrument_metric(&NET_ASSETS, portfolio, currency, instrument, asset.net_value);
            total.add(asset);
        }

        set_instrument_metric(&ASSETS, PORTFOLIO_LABEL_ALL, currency, instrument, total.value);
        set_instrument_metric(&NET_ASSETS, PORTFOLIO_LABEL_ALL, currency, instrument, total.net_value);
    }

    for method in PerformanceAnalysisMethod::iter() {
        let method_name: &str = method.into();
        let performance = statistics.performance(method);

        for (instrument, analysis) in &performance.instruments {
            if let Some(performance) = analysis.performance {
                set_performance_metric(&PERFORMANCE, currency, instrument, method_name, performance);
            }
        }

        if let Some(performance) = performance.portfolio.performance {
            set_performance_metric(&PERFORMANCE, currency, PORTFOLIO_INSTRUMENT, method_name, performance);
        }
    }

    set_portfolio_metric(&PROFIT, currency, income_structure.profit());
    set_portfolio_metric(&NET_PROFIT, currency, income_structure.net_profit);

    set_structure_metric(&INCOME_STRUCTURE, currency, "Trading", income_structure.net_trading_income());
    set_structure_metric(&INCOME_STRUCTURE, currency, "Dividends", income_structure.net_dividend_income());
    set_structure_metric(&INCOME_STRUCTURE, currency, "Interest", income_structure.net_interest_income());
    set_structure_metric(&INCOME_STRUCTURE, currency, "Tax deductions", income_structure.tax_deductions());

    set_structure_metric(&EXPENCES_STRUCTURE, currency, "Taxes", income_structure.taxes());
    set_structure_metric(&EXPENCES_STRUCTURE, currency, "Commissions", income_structure.commissions);

    set_portfolio_metric(&PROJECTED_TAXES, currency, statistics.projected_taxes);
    set_portfolio_metric(&PROJECTED_TAX_DEDUCTIONS, currency, statistics.projected_tax_deductions);
    set_portfolio_metric(&PROJECTED_COMMISSIONS, currency, statistics.projected_commissions);
}

fn collect_asset_groups(groups: &BTreeMap<String, AssetGroup>) {
    for (name, group) in groups {
        for value in &group.net_value {
            set_metric(&ASSET_GROUPS, &[name, value.currency], value.amount)
        }
    }
}

fn collect_lto(lto: &LtoStatistics) {
    for (year, result) in &lto.applied {
        let year = year.to_string();
        let year = year.as_str();

        set_metric(&LTO, &[year, "applied-above-limit"], result.applied_above_limit);
        set_metric(&LTO, &[year, "loss"], result.loss);
    }

    set_metric(&PROJECTED_LTO, &["deduction"], lto.projected.deduction);
    set_metric(&PROJECTED_LTO, &["limit"], lto.projected.limit);
    set_metric(&PROJECTED_LTO, &["loss"], lto.projected.loss);
}

fn collect_backtesting(results: &[BenchmarkBacktestingResult]) {
    let mut net_values = HashMap::new();

    for result in results {
        if let Some(previous) = net_values.insert((&result.name, &result.provider, &result.currency), result.net_value) {
            assert_eq!(previous, result.net_value)
        } else {
            set_metric(&BACKTESTING_NET_VALUE, &[
                &result.name, result.provider.as_deref().unwrap_or_default(), &result.currency,
            ], result.net_value);
        }

        if let Some(performance) = result.performance {
            set_metric(&BACKTESTING_PERFORMANCE, &[
                &result.name, result.provider.as_deref().unwrap_or_default(), result.method.into(), &result.currency,
            ], performance);
        }
    }
}

fn collect_forex_quotes(quotes: QuotesRc, pairs: &BTreeSet<String>) -> EmptyResult {
    quotes.batch_all(pairs.iter().map(|pair| {
        QuoteQuery::Forex(pair.to_owned())
    }))?;

    for pair in pairs {
        let (base, quote) = forex::parse_currency_pair(pair)?;
        let rate = quotes.get(QuoteQuery::Forex(pair.to_owned()))?;
        set_metric(&FOREX_PAIRS, &[base, quote], rate.amount);
    }

    Ok(())
}

fn save(path: &Path) -> EmptyResult {
    let encoder = TextEncoder::new();
    let metrics = prometheus::gather();

    let temp_path = util::temp_path(path);
    let mut file = BufWriter::new(File::create(&temp_path)?);

    encoder.encode(&metrics, &mut file)
        .map_err(Into::into)
        .and_then(|_| {
            Ok(file.flush()?)
        })
        .or_else(|err: GenericError| {
            fs::remove_file(&temp_path)?;
            Err(err)
        })?;

    Ok(fs::rename(&temp_path, path)?)
}

pub const NAMESPACE: &str = "investments";

pub const BACKTESTING_NET_VALUE_NAME: &str = "backtesting_net_value";
pub const BACKTESTING_PERFORMANCE_NAME: &str = "backtesting_performance";

const PORTFOLIO_LABEL: &str = "portfolio";
pub const PORTFOLIO_LABEL_ALL: &str = "all";

pub const PORTFOLIO_INSTRUMENT: &str = "Portfolio";

pub const CURRENCY_LABEL: &str = "currency";
pub const INSTRUMENT_LABEL: &str = "instrument";
pub const PROVIDER_LABEL: &str = "provider";
pub const TYPE_LABEL: &str = "type";

fn register_portfolio_metric(name: &str, help: &str) -> GaugeVec {
    register_metric(name, help, &[PORTFOLIO_LABEL, CURRENCY_LABEL])
}

fn register_instrument_metric(name: &str, help: &str) -> GaugeVec {
    register_metric(name, help, &[PORTFOLIO_LABEL, CURRENCY_LABEL, INSTRUMENT_LABEL])
}

fn register_performance_metric(name: &str, help: &str) -> GaugeVec {
    register_metric(name, help, &[PORTFOLIO_LABEL, CURRENCY_LABEL, INSTRUMENT_LABEL, TYPE_LABEL])
}

fn register_structure_metric(name: &str, help: &str) -> GaugeVec {
    register_metric(name, help, &[PORTFOLIO_LABEL, CURRENCY_LABEL, TYPE_LABEL])
}

fn register_metric(name: &str, help: &str, labels: &[&str]) -> GaugeVec {
    register_gauge_vec!(&format!("{}_{}", NAMESPACE, name), help, labels).unwrap()
}

fn register_simple_metric(name: &str, help: &str) -> Gauge {
    register_gauge!(&format!("{}_{}", NAMESPACE, name), help).unwrap()
}

fn set_portfolio_metric(collector: &GaugeVec, currency: &str, value: Decimal) {
    set_metric(collector, &[PORTFOLIO_LABEL_ALL, currency], value)
}

fn set_instrument_metric(collector: &GaugeVec, portfolio: &str, currency: &str, instrument: &str, value: Decimal) {
    set_metric(collector, &[portfolio, currency, instrument], value)
}

fn set_performance_metric(collector: &GaugeVec, currency: &str, instrument: &str, method: &str, value: Decimal) {
    set_metric(collector, &[PORTFOLIO_LABEL_ALL, currency, instrument, method], value)
}

fn set_structure_metric(collector: &GaugeVec, currency: &str, type_: &str, value: Decimal) {
    set_metric(collector, &[PORTFOLIO_LABEL_ALL, currency, type_], value)
}

fn set_metric(collector: &GaugeVec, labels: &[&str], value: Decimal) {
    collector.with_label_values(labels).set(value.to_f64().unwrap())
}