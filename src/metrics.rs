use std::io::{BufWriter, Write};
use std::fs::{self, File};

use lazy_static::lazy_static;
use num_traits::ToPrimitive;
use prometheus::{self, TextEncoder, Encoder, Gauge, GaugeVec, register_gauge, register_gauge_vec};

use crate::analysis::{self, PortfolioCurrencyStatistics, LtoStatistics};
use crate::config::Config;
use crate::core::{EmptyResult, GenericError, GenericResult};
use crate::currency::converter::CurrencyConverter;
use crate::telemetry::TelemetryRecordBuilder;
use crate::time;
use crate::types::Decimal;

lazy_static! {
    static ref UPDATE_TIME: Gauge = register_simple_metric(
        "time", "Metrics generation time");

    static ref BROKERS: GaugeVec = register_metric(
        "brokers", "Net asset value by broker", &["currency", "broker", "country"]);

    static ref ASSETS: GaugeVec = register_instrument_metric(
        "assets", "Open positions value");

    static ref PERFORMANCE: GaugeVec = register_instrument_metric(
        "performance", "Instrument performance");

    static ref INCOME_STRUCTURE: GaugeVec = register_structure_metric(
        "income_structure", "Income structure");

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

pub fn collect(config: &Config, path: &str) -> GenericResult<TelemetryRecordBuilder> {
    let (statistics, converter, telemetry) = analysis::analyse(
        config, None, false, Some(&config.metrics.merge_performance), false)?;

    UPDATE_TIME.set(cast::f64(time::utc_now().timestamp()));

    for statistics in &statistics.currencies {
        collect_portfolio_metrics(statistics);
    }

    collect_lto_metrics(statistics.lto.as_ref().unwrap());
    collect_forex_quotes(&converter, "USD", "RUB")?;

    save(path)?;

    Ok(telemetry)
}

fn collect_portfolio_metrics(statistics: &PortfolioCurrencyStatistics) {
    let currency = &statistics.currency;
    let performance = statistics.performance.as_ref().unwrap();
    let income_structure = &performance.income_structure;

    for (broker, &value) in &statistics.brokers {
        set_metric(&BROKERS, &[currency, broker.brief_name(), broker.jurisdiction().name()], value);
    }

    for (instrument, &value) in &statistics.assets {
        set_instrument_metric(&ASSETS, currency, instrument, value);
    }

    for (instrument, analysis) in &performance.instruments {
        if let Some(interest) = analysis.interest {
            set_instrument_metric(&PERFORMANCE, currency, instrument, interest);
        }
    }

    if let Some(interest) = performance.portfolio.interest {
        set_instrument_metric(&PERFORMANCE, currency, "Portfolio", interest);
    }

    set_portfolio_metric(&PROFIT, currency, income_structure.profit());
    set_portfolio_metric(&NET_PROFIT, currency, income_structure.net_profit);

    set_structure_metric(&INCOME_STRUCTURE, currency, "Trading", income_structure.trading());
    set_structure_metric(&INCOME_STRUCTURE, currency, "Dividends", income_structure.dividends);
    set_structure_metric(&INCOME_STRUCTURE, currency, "Interest", income_structure.interest);
    set_structure_metric(&INCOME_STRUCTURE, currency, "Tax deductions", income_structure.tax_deductions);

    set_structure_metric(&EXPENCES_STRUCTURE, currency, "Taxes", income_structure.taxes);
    set_structure_metric(&EXPENCES_STRUCTURE, currency, "Commissions", income_structure.commissions);

    set_portfolio_metric(&PROJECTED_TAXES, currency, statistics.projected_taxes);
    set_portfolio_metric(&PROJECTED_TAX_DEDUCTIONS, currency, statistics.projected_tax_deductions);
    set_portfolio_metric(&PROJECTED_COMMISSIONS, currency, statistics.projected_commissions);
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

fn set_structure_metric(collector: &GaugeVec, currency: &str, type_: &str, value: Decimal) {
    set_metric(collector, &[PORTFOLIO_LABEL_ALL, currency, type_], value)
}

fn set_metric(collector: &GaugeVec, labels: &[&str], value: Decimal) {
    collector.with_label_values(labels).set(value.to_f64().unwrap())
}