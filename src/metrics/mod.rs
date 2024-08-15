pub mod config;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufWriter, Write};
use std::fs::{self, File};
use std::path::Path;

use lazy_static::lazy_static;
use num_traits::ToPrimitive;
use prometheus::{self, TextEncoder, Encoder, Gauge, GaugeVec, register_gauge, register_gauge_vec};
use strum::IntoEnumIterator;

use crate::analysis::{self, PerformanceAnalysisMethod};
use crate::analysis::portfolio_statistics::{Asset, AssetGroup, PortfolioCurrencyStatistics, LtoStatistics};
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
        "brokers", "Net asset value by broker", &["currency", "broker", "country"]);

    static ref ASSETS: GaugeVec = register_instrument_metric(
        "assets", "Open positions value");

    static ref NET_ASSETS: GaugeVec = register_instrument_metric(
        "net_assets", "Open positions net value");

    static ref ASSET_GROUPS: GaugeVec = register_metric(
        "asset_groups", "Net asset value of custom groups", &["name", "currency"]);

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
        "lto", "Long-term ownership tax exemption applying results", &["year", "type"]);

    static ref PROJECTED_LTO: GaugeVec = register_metric(
        "projected_lto", "Long-term ownership tax exemption projected results", &["type"]);

    static ref FOREX_PAIRS: GaugeVec = register_metric(
        "forex_pairs", "Forex quotes", &["base", "quote"]);
}

pub fn collect(config: &Config, path: &Path) -> GenericResult<TelemetryRecordBuilder> {
    let (statistics, quotes, telemetry) = analysis::analyse(
        config, None, false, &config.metrics.asset_groups,
        Some(&config.metrics.merge_performance), false)?;

    UPDATE_TIME.set(cast::f64(time::timestamp()));

    for statistics in &statistics.currencies {
        collect_portfolio_metrics(statistics);
    }

    collect_forex_quotes(quotes, &config.metrics.currency_rates)?;
    collect_asset_groups(&statistics.asset_groups);
    collect_lto_metrics(statistics.lto.as_ref().unwrap());

    save(path)?;

    Ok(telemetry)
}

fn collect_portfolio_metrics(statistics: &PortfolioCurrencyStatistics) {
    let currency = &statistics.currency;
    let income_structure = &statistics.real_performance.as_ref().unwrap().income_structure;

    for (broker, &value) in &statistics.brokers {
        set_metric(&BROKERS, &[currency, broker.brief_name(), broker.jurisdiction().traits().name], value);
    }

    for (instrument, portfolios) in &statistics.assets {
        let mut total = Asset::default();

        // TODO(konishchev): Split by portfolio
        for asset in portfolios.values() {
            total.add(asset);
        }

        set_instrument_metric(&ASSETS, currency, instrument, total.value);
        set_instrument_metric(&NET_ASSETS, currency, instrument, total.net_value);
    }

    for method in PerformanceAnalysisMethod::iter() {
        let method_name: &str = method.into();
        let performance = statistics.performance(method);

        for (instrument, analysis) in &performance.instruments {
            if let Some(interest) = analysis.interest {
                set_performance_metric(&PERFORMANCE, currency, instrument, method_name, interest);
            }
        }

        if let Some(interest) = performance.portfolio.interest {
            set_performance_metric(&PERFORMANCE, currency, "Portfolio", method_name, interest);
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

fn collect_lto_metrics(lto: &LtoStatistics) {
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

const NAMESPACE: &str = "investments";

const PORTFOLIO_LABEL: &str = "portfolio";
const PORTFOLIO_LABEL_ALL: &str = "all";

const CURRENCY_LABEL: &str = "currency";

fn register_portfolio_metric(name: &str, help: &str) -> GaugeVec {
    register_metric(name, help, &[PORTFOLIO_LABEL, CURRENCY_LABEL])
}

fn register_instrument_metric(name: &str, help: &str) -> GaugeVec {
    register_metric(name, help, &[PORTFOLIO_LABEL, CURRENCY_LABEL, "instrument"])
}

fn register_performance_metric(name: &str, help: &str) -> GaugeVec {
    register_metric(name, help, &[PORTFOLIO_LABEL, CURRENCY_LABEL, "instrument", "type"])
}

fn register_structure_metric(name: &str, help: &str) -> GaugeVec {
    register_metric(name, help, &[PORTFOLIO_LABEL, CURRENCY_LABEL, "type"])
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

fn set_instrument_metric(collector: &GaugeVec, currency: &str, instrument: &str, value: Decimal) {
    set_metric(collector, &[PORTFOLIO_LABEL_ALL, currency, instrument], value)
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