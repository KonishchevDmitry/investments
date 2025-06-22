mod benchmark;
pub mod config;
mod deposit;
mod stock;

use std::borrow::Cow;
use std::collections::BTreeMap;

use chrono::Duration;
use easy_logging::GlobalContext;
use itertools::Itertools;
use log::{debug, warn};
use num_traits::ToPrimitive;
use static_table_derive::StaticTable;
use strum::IntoEnumIterator;

use crate::analysis::deposit::{InterestPeriod, Transaction};
use crate::analysis::inflation::InflationCalc;
use crate::broker_statement::{BrokerStatement, StockSellType, StockSource};
use crate::core::GenericResult;
use crate::currency::{Cash, CashAssets};
use crate::currency::converter::{CurrencyConverter, CurrencyConverterRc};
use crate::formatting;
use crate::formatting::table::Cell;
use crate::metrics::{self, backfilling::DailyTimeSeries};
use crate::quotes::QuotesRc;
use crate::time::{self, Date};
use crate::types::Decimal;

use self::benchmark::{Benchmark, BacktestingResult};
use self::config::{BacktestingConfig};
use self::deposit::DepositBenchmark;
use self::stock::StockBenchmark;

pub struct BenchmarkBacktestingResult {
    pub name: String,
    pub provider: Option<String>,

    pub method: BenchmarkPerformanceType,
    pub currency: String,

    pub net_value: Decimal,
    pub performance: Option<Decimal>,
}

pub fn backtest(
    config: &BacktestingConfig, statements: &[BrokerStatement], converter: CurrencyConverterRc, quotes: QuotesRc,
    with_metrics: Option<Duration>, interactive: Option<BenchmarkPerformanceType>,
) -> GenericResult<(Vec<BenchmarkBacktestingResult>, Vec<DailyTimeSeries>)> {
    let currencies = ["USD", "RUB"];

    let mut benchmarks = Vec::<Box<dyn Benchmark>>::new();
    for benchmark in &config.benchmarks {
        benchmarks.push(Box::new(StockBenchmark::new(benchmark, converter.clone(), quotes.clone())?));
    }

    if interactive.is_some() && benchmarks.is_empty() {
        warn!("There are no benchmark specs in the configuration file.");
    }

    for statement in statements {
        if interactive.is_some() {
            statement.check_date();
        }
        statement.batch_quotes(&quotes)?;
    }

    let cash_flows = generalize_portfolios_to_cash_flows(statements);
    let start_date = cash_flows.first().ok_or(
        "An attempt to backtest an empty portfolio (without cash flows)")?.date;

    let today = time::today();
    if start_date == today {
        return Err!("An attempt to backtest portfolio which was created today");
    } else if today < cash_flows.last().unwrap().date {
        return Err!("The portfolio contains future cash flows");
    }

    let with_metrics = with_metrics.map(|min_performance_period| {
        start_date.checked_add_signed(min_performance_period).unwrap_or(Date::MAX)
    });

    for currency in currencies {
        let infinity = (Date::MAX - start_date).num_days().try_into().unwrap();
        benchmarks.push(Box::new(DepositBenchmark::new(
            &format!("{currency} cash"), currency, infinity, infinity, converter.clone())));
    }

    let interest_periods = [InterestPeriod::new(start_date, today)];
    let days = crate::analysis::deposit::get_total_activity_duration(&interest_periods);
    let format_performance = |performance| format!("{}%", performance);

    let mut results = Vec::new();
    let mut daily_time_series = Vec::new();

    for (method_index, method) in BenchmarkPerformanceType::iter().enumerate() {
        for currency in currencies {
            let mut table = interactive.and_then(|interactive_method| {
                if method == interactive_method {
                    Some(BacktestingResultsTable::new())
                } else {
                    None
                }
            });

            {
                let name = metrics::PORTFOLIO_INSTRUMENT;
                let _logging_context = GlobalContext::new(&format!("{name} / {method} {currency}"));

                let transactions = cash_flows_to_transactions(currency, &cash_flows, &converter)?;

                let mut net_value = Cash::zero(currency);
                for statement in statements {
                    net_value += statement.net_value(&converter, &quotes, currency, true)?;
                }

                let result = BacktestingResult::calculate(name, today, net_value, method, &transactions, true)?;

                if let Some(table) = table.as_mut() {
                    table.add_row(BacktestingResultsTableRow {
                        benchmark: name.to_owned(),
                        result: Cell::new_round_decimal(net_value.amount),
                        performance: result.performance.map(format_performance),
                    });
                }

                results.push(BenchmarkBacktestingResult {
                    name: name.to_owned(),
                    provider: None,

                    method,
                    currency: currency.to_owned(),

                    net_value: result.net_value,
                    performance: result.performance,
                });
            };

            for benchmark in &benchmarks {
                let full_name = format!("{} / {method} {currency}", benchmark.full_name());
                let _logging_context = GlobalContext::new(&full_name);

                debug!("Backtesting {}...", benchmark.full_name());

                let daily_results = benchmark.backtest(
                    method, currency, &cash_flows, today, with_metrics,
                ).map_err(|e| format!("Failed to backtest the portfolio against {} benchmark: {e}", benchmark.full_name()))?;

                if let Some(performance_start_date) = with_metrics {
                    let metrics = generate_metrics(
                        benchmark.as_ref(), method, currency, &daily_results, performance_start_date, method_index == 0
                    ).map_err(|e| format!("Failed to generate metrics for {full_name}: {e}"))?;
                    daily_time_series.extend(metrics);
                }

                let current = daily_results.last().unwrap();
                assert_eq!(current.date, today);

                results.push(BenchmarkBacktestingResult {
                    name: benchmark.name(),
                    provider: benchmark.provider(),

                    method,
                    currency: currency.to_owned(),

                    net_value: current.net_value,
                    performance: current.performance,
                });

                if let Some(table) = table.as_mut() {
                    table.add_row(BacktestingResultsTableRow {
                        benchmark: benchmark.full_name(),
                        result: Cell::new_round_decimal(current.net_value),
                        performance: current.performance.map(format_performance),
                    });
                }
            }

            if let Some(table) = table.as_mut() {
                table.print(&format!("Backtesting results ({currency}, {})", formatting::format_days(days)));
            }
        }
    }

    Ok((results, daily_time_series))
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[derive(strum::Display, strum::EnumIter, strum::EnumString, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
pub enum BenchmarkPerformanceType {
    Virtual,
    InflationAdjusted,
}

impl BenchmarkPerformanceType {
    fn adjust_transactions<'t>(
        self, currency: &str, today: Date, transactions: &'t [Transaction],
    ) -> GenericResult<Cow<'t, [Transaction]>> {
        let inflation_calc = match self {
            BenchmarkPerformanceType::InflationAdjusted => {
                InflationCalc::new(currency, today)?
            },
            BenchmarkPerformanceType::Virtual => {
                return Ok(Cow::Borrowed(transactions));
            },
        };

        Ok(Cow::Owned(transactions.iter().map(|transaction| {
            let amount = inflation_calc.adjust(transaction.date, transaction.amount);
            Transaction::new(transaction.date, amount)
        }).collect()))
    }
}

#[derive(StaticTable)]
#[table(name="BacktestingResultsTable")]
struct BacktestingResultsTableRow {
    #[column(name="Benchmark")]
    benchmark: String,
    #[column(name="Result")]
    result: Cell,
    #[column(name="Performance", align="right")]
    performance: Option<String>,
}

fn generalize_portfolios_to_cash_flows(statements: &[BrokerStatement]) -> Vec<CashAssets> {
    let mut cash_flows = BTreeMap::new();
    let mut add = |date: Date, cash: Cash| {
        cash_flows.entry((date, cash.currency))
            .and_modify(|total| *total += cash)
            .or_insert(cash);
    };

    for statement in statements {
        generalize_portfolio_to_cash_flows(statement, &mut add)
    }

    cash_flows.into_iter().filter_map(|((date, _currency), cash)| {
        if cash.is_zero() {
            None
        } else {
            Some(CashAssets::new_from_cash(date, cash))
        }
    }).collect_vec()
}

fn generalize_portfolio_to_cash_flows<A: FnMut(Date, Cash)>(statement: &BrokerStatement, mut add: A) {
    for cash_flow in &statement.deposits_and_withdrawals {
        add(cash_flow.date, cash_flow.cash);
    }

    for trade in &statement.forex_trades {
        add(trade.conclusion_time.date, -trade.commission)
    }

    for trade in &statement.stock_buys {
        match trade.type_ {
            StockSource::Trade { commission, .. } => {
                add(trade.conclusion_time.date, -commission);
            },
            StockSource::CorporateAction | StockSource::Grant => {},
        }
    }

    for trade in &statement.stock_sells {
        match trade.type_ {
            StockSellType::Trade { commission, .. } => {
                add(trade.conclusion_time.date, -commission);
            },
            StockSellType::CorporateAction => {},
        }
    }

    for fee in &statement.fees {
        add(fee.date, -fee.amount.withholding());
    }

    for tax in &statement.tax_agent_withholdings {
        add(tax.date, -tax.amount.withholding())
    }
}

fn cash_flows_to_transactions(currency: &str, cash_flows: &[CashAssets], converter: &CurrencyConverter) -> GenericResult<Vec<Transaction>> {
    let mut transactions = BTreeMap::new();

    for cash_flow in cash_flows {
        let amount = converter.convert_to(cash_flow.date, cash_flow.cash, currency)?;
        transactions.entry(cash_flow.date)
            .and_modify(|total| *total += amount)
            .or_insert(amount);
    }

    Ok(transactions.into_iter().map(|(date, amount)| {
        Transaction::new(date, amount)
    }).collect_vec())
}

fn generate_metrics(
    benchmark: &dyn Benchmark, method: BenchmarkPerformanceType, currency: &str, results: &[BacktestingResult],
    performance_start_date: Date, with_net_value: bool,
) -> GenericResult<Vec<DailyTimeSeries>> {
    let time_series = |name: &str| {
        DailyTimeSeries::new(&format!("{}_{name}", metrics::NAMESPACE))
            .with_label(metrics::INSTRUMENT_LABEL, &benchmark.name())
            .with_label(metrics::PROVIDER_LABEL, benchmark.provider().as_deref().unwrap_or_default())
            .with_label(metrics::CURRENCY_LABEL, currency)
    };

    let mut net_value_time_series = with_net_value.then(|| time_series(metrics::BACKTESTING_NET_VALUE_NAME));
    let mut performance_time_series = time_series(metrics::BACKTESTING_PERFORMANCE_NAME)
        .with_label(metrics::TYPE_LABEL, method.into());

    for result in results {
        if let Some(net_value_time_series) = net_value_time_series.as_mut() {
            let net_value = result.net_value.to_f64().ok_or_else(|| format!(
                "Got an invalid result: {}", result.net_value))?;
            net_value_time_series.add_value(result.date, net_value);
        }

        if let Some(performance) = result.performance {
            if result.date >= performance_start_date {
                let performance = performance.to_f64().ok_or_else(|| format!(
                    "Got an invalid performance: {}", performance))?;
                performance_time_series.add_value(result.date, performance);
            }
        }
    }

    let mut metrics = Vec::new();

    if let Some(net_value_time_series) = net_value_time_series {
        metrics.push(net_value_time_series);
    }

    if !performance_time_series.is_empty() {
        metrics.push(performance_time_series);
    }

    Ok(metrics)
}