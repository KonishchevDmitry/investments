use std::io::{BufWriter, Write};
use std::fs::{self, File};
use std::time::SystemTime;

use lazy_static::lazy_static;
use num_traits::ToPrimitive;
use prometheus::{self, TextEncoder, Encoder, Gauge, GaugeVec, register_gauge, register_gauge_vec};

use crate::analysis::{self, PortfolioCurrencyStatistics};
use crate::config::Config;
use crate::core::{EmptyResult, GenericError};
use crate::currency::converter::CurrencyConverter;
use crate::types::Decimal;

lazy_static! {
    static ref UPDATE_TIME: Gauge = register_simple_metric(
        "time", "Metrics generation time.");

    static ref ASSETS: GaugeVec = register_instrument_metric(
        "assets", "Open positions value.");

    static ref PERFORMANCE: GaugeVec = register_instrument_metric(
        "performance", "Instrument performance.");

    static ref INCOME_STRUCTURE: GaugeVec = register_metric(
        "income_structure", "Income structure.", &[CURRENCY_LABEL, "type"]);

    static ref EXPENCES_STRUCTURE: GaugeVec = register_metric(
        "expenses_structure", "Expenses structure.", &[CURRENCY_LABEL, "type"]);

    static ref PROFIT: GaugeVec = register_portfolio_metric(
        "profit", "Profit.");

    static ref NET_PROFIT: GaugeVec = register_portfolio_metric(
        "net_profit", "Net profit.");

    static ref PROJECTED_TAXES: GaugeVec = register_portfolio_metric(
        "projected_taxes", "Projected taxes to pay.");

    static ref PROJECTED_COMMISSIONS: GaugeVec = register_portfolio_metric(
        "projected_commissions", "Projected commissions to pay.");

    static ref FOREX_PAIRS: GaugeVec = register_metric(
        "forex_pairs", "Forex quotes.", &["base", "quote"]);
}

// FIXME(konishchev): Regression tests
pub fn collect(config: &Config, path: &str) -> EmptyResult {
    let (statistics, converter) = analysis::analyse(
        config, None, false, Some(&config.metrics.merge_performance), false)?;

    let update_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
    UPDATE_TIME.set(cast::f64(update_time));

    for statistics in statistics.currencies {
        collect_portfolio_metrics(&statistics);
    }

    collect_forex_quotes(&converter, "USD", "RUB")?;

    save(path)
}

fn collect_portfolio_metrics(statistics: &PortfolioCurrencyStatistics) {
    let currency = &statistics.currency;
    let performance = statistics.performance.as_ref().unwrap();
    let income_structure = &performance.income_structure;

    for (instrument, &value) in &statistics.assets {
        set_instrument_metric(&ASSETS, currency, &instrument, value);
    }

    for (instrument, analysis) in &performance.instruments {
        set_instrument_metric(&PERFORMANCE, currency, &instrument, analysis.interest);
    }
    set_instrument_metric(&PERFORMANCE, currency, "Portfolio", performance.portfolio.interest);

    set_portfolio_metric(&PROFIT, currency, income_structure.profit());
    set_portfolio_metric(&NET_PROFIT, currency, income_structure.net_profit);

    set_metric(&INCOME_STRUCTURE, &[currency, "Trading"], income_structure.trading());
    set_metric(&INCOME_STRUCTURE, &[currency, "Dividends"], income_structure.dividends);
    set_metric(&INCOME_STRUCTURE, &[currency, "Interest"], income_structure.interest);
    set_metric(&INCOME_STRUCTURE, &[currency, "Tax deductions"], income_structure.tax_deductions);

    set_metric(&EXPENCES_STRUCTURE, &[currency, "Taxes"], income_structure.taxes);
    set_metric(&EXPENCES_STRUCTURE, &[currency, "Commissions"], income_structure.commissions);

    set_portfolio_metric(&PROJECTED_TAXES, currency, statistics.projected_taxes);
    set_portfolio_metric(&PROJECTED_COMMISSIONS, currency, statistics.projected_commissions);
}

fn collect_forex_quotes(converter: &CurrencyConverter, base: &str, quote: &str) -> EmptyResult {
    Ok(set_metric(&FOREX_PAIRS, &[base, quote], converter.real_time_currency_rate(base, quote)?))
}

fn save(path: &str) -> EmptyResult {
    let encoder = TextEncoder::new();
    let metrics = prometheus::gather();

    let temp_path = format!("{}.tmp", path);
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

fn set_metric(collector: &GaugeVec, labels: &[&str], value: Decimal) {
    collector.with_label_values(labels).set(value.to_f64().unwrap())
}