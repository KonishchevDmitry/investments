use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Bound;

use easy_logging::GlobalContext;
use itertools::Itertools;
use log::debug;
use num_traits::ToPrimitive;
use static_table_derive::StaticTable;
use strum::IntoEnumIterator;

use crate::analysis::deposit_emulator::{InterestPeriod, Transaction};
use crate::analysis::deposit_performance;
use crate::analysis::inflation::InflationCalc;
use crate::analysis::portfolio_performance;
use crate::broker_statement::{BrokerStatement, StockSellType, StockSource};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets, MultiCurrencyCashAccount};
use crate::currency::converter::{CurrencyConverter, CurrencyConverterRc};
use crate::exchanges::Exchange;
use crate::formatting;
use crate::formatting::table::Cell;
use crate::metrics::{self, backfilling::DailyTimeSeries};
use crate::quotes::{Quotes, QuotesRc, QuoteQuery, HistoricalQuotes};
use crate::time::{self, Date, Period};
use crate::types::Decimal;

pub struct BacktestingResults {
    pub method: BenchmarkPerformanceType,
    pub currency: String,

    pub portfolio_net_value: Decimal,
    pub portfolio_performance: Option<Decimal>,

    pub benchmark_metrics: Option<Vec<DailyTimeSeries>>,
}

pub fn backtest(
    statements: &[BrokerStatement], benchmarks: Option<&[Benchmark]>, converter: CurrencyConverterRc, quotes: QuotesRc,
    with_metrics: bool, interactive: Option<BenchmarkPerformanceType>,
) -> GenericResult<Vec<BacktestingResults>> {
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

    let interest_periods = [InterestPeriod::new(start_date, today)];
    let days = portfolio_performance::get_total_activity_duration(&interest_periods);
    let format_performance = |performance| format!("{}%", performance);

    let mut results = Vec::new();

    for method in BenchmarkPerformanceType::iter() {
        for currency in ["USD", "RUB"] {
            let mut table = interactive.and_then(|interactive_method| {
                if method == interactive_method {
                    Some(BacktestingResultsTable::new())
                } else {
                    None
                }
            });

            let mut result = {
                let _logging_context = GlobalContext::new(&format!("Portfolio / {method} {currency}"));

                let transactions = cash_flows_to_transactions(currency, &cash_flows, &converter)?;
                let transactions = method.adjust_transactions(currency, today, &transactions)?;

                let mut net_value = Cash::zero(currency);
                for statement in statements {
                    net_value += statement.net_value(&converter, &quotes, currency, true)?;
                }

                let performance = deposit_performance::compare_instrument_to_bank_deposit(
                    "portfolio", currency, &transactions, &interest_periods, net_value.amount)?;

                if let Some(table) = table.as_mut() {
                    table.add_row(BacktestingResultsTableRow {
                        benchmark: s!("Portfolio"),
                        result: Cell::new_round_decimal(net_value.amount),
                        performance: performance.map(format_performance),
                    });
                }

                BacktestingResults {
                    method,
                    currency: currency.to_owned(),

                    portfolio_net_value: net_value.round().amount,
                    portfolio_performance: performance,

                    benchmark_metrics: None,
                }
            };

            for benchmark in benchmarks.unwrap_or_default() {
                let full_name = format!("{} / {method} {currency}", benchmark.name());
                let _logging_context = GlobalContext::new(&full_name);

                let daily_results = benchmark.backtest(
                    method, currency, &cash_flows, with_metrics, today, converter.clone(), quotes.clone(),
                ).map_err(|e| format!("Failed to backtest the portfolio against {} benchmark: {e}", benchmark.name()))?;

                if with_metrics {
                    let metrics = generate_metrics(benchmark, method, currency, &daily_results).map_err(|e| format!(
                        "Failed to generate metrics for {full_name}: {e}"))?;
                    result.benchmark_metrics.get_or_insert_default().extend(metrics);
                }

                if let Some(table) = table.as_mut() {
                    let current = daily_results.last().unwrap();
                    assert_eq!(current.date, today);

                    table.add_row(BacktestingResultsTableRow {
                        benchmark: benchmark.name(),
                        result: Cell::new_round_decimal(current.net_value),
                        performance: current.performance.map(format_performance),
                    });
                }
            }

            if let Some(table) = table.as_mut() {
                table.print(&format!("Backtesting results ({currency}, {})", formatting::format_days(days)));
            }

            results.push(result);
        }
    }

    Ok(results)
}

pub struct Benchmark {
    name: String,
    provider: Option<String>,
    instruments: BTreeMap<Date, BenchmarkInstrument>
}

impl Benchmark {
    pub fn new(name: &str, instrument: BenchmarkInstrument) -> Benchmark {
        Benchmark {
            name: name.to_owned(),
            provider: None,
            instruments: btreemap!{
                Date::MIN => instrument,
            }
        }
    }

    pub fn with_provider(mut self, name: &str) -> Self {
        self.provider.replace(name.to_owned());
        self
    }

    pub fn then(self, date: Date, instrument: BenchmarkInstrument) -> GenericResult<Self> {
        self.then_transition(date, instrument)
    }

    pub fn then_rename(self, date: Date, mut instrument: BenchmarkInstrument) -> GenericResult<Self> {
        instrument.renamed_from = Some(self.instruments.last_key_value().unwrap().1.id.clone());
        self.then_transition(date, instrument)
    }

    pub fn name(&self) -> String {
        match self.provider.as_ref() {
            Some(provider) => format!("{} ({provider})", self.name),
            None => self.name.clone(),
        }
    }

    fn backtest(
        &self, method: BenchmarkPerformanceType, currency: &str, cash_flows: &[CashAssets], full: bool,
        today: Date, converter: CurrencyConverterRc, quotes: QuotesRc,
    ) -> GenericResult<Vec<BacktestingResult>> {
        debug!("Backtesting {}...", self.name());

        let start_date = cash_flows.first().unwrap().date;

        let quotes_limit = if full {
            today.pred_opt().unwrap()
        } else {
            cash_flows.last().unwrap().date
        };

        let (transition_date, instrument) = self.select_instrument(start_date, quotes_limit, &quotes, true)?;
        assert_eq!(transition_date, start_date);

        Backtester {
            method, currency, quotes, converter, quotes_limit,

            benchmark: self,
            cash_flows,
            transactions: Vec::new(),
            results: Vec::new(),
            full,

            date: start_date,
            today,

            cash_assets: MultiCurrencyCashAccount::new(),
            instrument: instrument
        }.backtest()
    }

    fn then_transition(mut self, date: Date, instrument: BenchmarkInstrument) -> GenericResult<Self> {
        let &last_date = self.instruments.last_key_value().unwrap().0;

        if date == last_date {
            return Err!("An attempt to override {}", formatting::format_date(date));
        } else if date < last_date {
            return Err!("Benchmark instruments chain must be ordered by date");
        }

        assert!(self.instruments.insert(date, instrument).is_none());
        Ok(self)
    }

    fn select_instrument(&self, date: Date, quotes_limit: Date, quotes: &Quotes, first: bool) -> GenericResult<(Date, CurrentBenchmarkInstrument)> {
        let Some((&start_date, instrument)) = self.instruments.range(..=date).last() else {
            return Err!("There is no benchmark instrument for {}", formatting::format_date(date));
        };

        // We don't select the passed date as instrument transition date, because it's highly likely that instrument
        // transition date was carefully selected as a date with minimum volatility for this instrument.
        //
        // But if it's the first instrument, there will be no transition actually, so we can narrow quotes period.
        let transition_date = if first {
            date
        } else {
            start_date
        };

        let until = self.instruments.range((Bound::Excluded(start_date), Bound::Unbounded)).next()
            .map(|(date, _instrument)| date).copied();

        let quotes_period = Period::new(
            instrument.id.exchange.min_last_working_day(transition_date),
            std::cmp::max(transition_date, until.unwrap_or(quotes_limit)),
        )?;

        debug!(
            "Select new benchmark instrument for {}+: {} ({quotes_period}).",
            formatting::format_date(date), instrument.id.symbol);

        let mut candles = HistoricalQuotes::new();
        for symbol in [&instrument.id.symbol].into_iter().chain(instrument.aliases.iter()) {
            candles = quotes.get_historical(instrument.id.exchange, symbol, quotes_period)?;
            if !candles.is_empty() {
                break
            }
        }

        Ok((transition_date, CurrentBenchmarkInstrument {
            spec: instrument,
            quotes: candles,
            until,
            quantity: dec!(0),
        }))
    }
}

struct Backtester<'a> {
    method: BenchmarkPerformanceType,
    currency: &'a str,
    quotes: QuotesRc,
    converter: CurrencyConverterRc,
    quotes_limit: Date,

    benchmark: &'a Benchmark,
    cash_flows: &'a [CashAssets],
    transactions: Vec<Transaction>,
    results: Vec<BacktestingResult>,
    full: bool,

    date: Date,
    today: Date,

    cash_assets: MultiCurrencyCashAccount,
    instrument: CurrentBenchmarkInstrument<'a>,
}

impl Backtester<'_> {
    fn backtest(mut self) -> GenericResult<Vec<BacktestingResult>> {
        for cash_flow in self.cash_flows {
            debug!("Backtesting cash flow: {}: {}...", formatting::format_date(cash_flow.date), cash_flow.cash);
            self.process_to(cash_flow.date)?;

            let amount = self.converter.convert_to(cash_flow.date, cash_flow.cash, self.currency)?;
            self.transactions.push(Transaction::new(cash_flow.date, amount));

            self.cash_assets.deposit(cash_flow.cash);
            self.instrument.process_cash_flow(cash_flow.date, &mut self.cash_assets, &self.converter)?;
        }

        self.process_to(self.today)?;
        self.close_day()?;

        Ok(self.results)
    }

    fn process_to(&mut self, date: Date) -> EmptyResult {
        assert!(date >= self.date);

        while self.date != date {
            if self.full {
                self.close_day()?; // Steps to the next day
            } else {
                self.date = date; // Attention: jumps to the requested date possibly skipping some transitions
            }

            let Some(transition_date) = self.instrument.until else {
                continue;
            };

            if self.date < transition_date {
                continue;
            }

            let (transition_date, mut new_instrument) = self.benchmark.select_instrument(
                self.date, self.quotes_limit, &self.quotes, false)?;

            if self.full {
                assert_eq!(transition_date, self.date);
            }

            match new_instrument.spec.renamed_from.as_ref() {
                Some(renamed_from) if *renamed_from == self.instrument.spec.id => {
                    new_instrument.quantity = self.instrument.quantity;
                },
                _ => {
                    self.cash_assets.deposit(self.instrument.get_value(transition_date)?);
                    new_instrument.process_cash_flow(transition_date, &mut self.cash_assets, &self.converter.clone())?;
                },
            }

            debug!("Converted {} {} -> {} {} at {}.",
                self.instrument.quantity, self.instrument.spec.id.symbol,
                new_instrument.quantity, new_instrument.spec.id.symbol,
                formatting::format_date(transition_date));

            self.instrument = new_instrument;
        }

        Ok(())
    }

    fn close_day(&mut self) -> EmptyResult {
        assert!(self.date <= self.today);

        let net_value = if self.date == self.today {
            let cash_assets = self.cash_assets.total_assets_real_time(self.currency, &self.converter)?;

            let instrument = &self.instrument.spec.id;
            let price = self.quotes.get(QuoteQuery::Stock(instrument.symbol.clone(), vec![instrument.exchange]))?;
            let instrument_value = self.converter.real_time_convert_to(price * self.instrument.quantity, self.currency)?;

            cash_assets + instrument_value
        } else {
            let mut net_value = MultiCurrencyCashAccount::new();
            net_value.add(&self.cash_assets);
            net_value.deposit(self.instrument.get_value(self.date)?);
            net_value.total_assets(self.date, self.currency, &self.converter)?
        };

        let mut result = BacktestingResult {
            date: self.date,
            net_value,
            performance: None,
        };

        let start_date = self.transactions.first().unwrap().date;
        if (self.date - start_date).num_days() > 0 { // FIXME(konishchev): Alter the value
            let name = format!("{} @ {}", self.benchmark.name(), formatting::format_date(self.date));

            let transactions = self.method.adjust_transactions(self.currency, self.date, &self.transactions)?;
            let interest_periods = [InterestPeriod::new(start_date, self.date)];

            result.performance = deposit_performance::compare_instrument_to_bank_deposit(
                &name, self.currency, &transactions, &interest_periods, net_value)?;
        }

        self.results.push(result);
        self.date = self.date.succ_opt().unwrap();

        Ok(())
    }
}

#[derive(Clone, PartialEq)]
pub struct BenchmarkInstrumentId {
    symbol: String,
    exchange: Exchange,
}

pub struct BenchmarkInstrument {
    id: BenchmarkInstrumentId,
    aliases: Vec<String>,
    renamed_from: Option<BenchmarkInstrumentId>,
}

impl BenchmarkInstrument {
    pub fn new(symbol: &str, exchange: Exchange) -> Self {
        BenchmarkInstrument {
            id: BenchmarkInstrumentId {
                symbol: symbol.to_owned(),
                exchange,
            },
            aliases: Vec::new(),
            renamed_from: None,
        }
    }

    // Historical API handle instrument renames differently:
    // * With MOEX API you need to request quotes for different symbols depending on the period.
    // * T-Bank API forgets all previous instrument symbols and returns all quotes by its current symbol.
    //
    // This method is used to write search rules which will work with both provider types.
    pub fn alias(mut self, symbol: &str) -> Self {
        self.aliases.push(symbol.to_owned());
        self
    }
}

struct CurrentBenchmarkInstrument<'a> {
    spec: &'a BenchmarkInstrument,
    quotes: HistoricalQuotes,
    until: Option<Date>,
    quantity: Decimal,
}

impl CurrentBenchmarkInstrument<'_> {
    fn process_cash_flow(
        &mut self, date: Date, cash_assets: &mut MultiCurrencyCashAccount, converter: &CurrencyConverter,
    ) -> EmptyResult {
        let price = self.get_price(date)?;
        let cash = cash_assets.total_cash_assets(date, price.currency, converter)?;

        self.quantity += cash / price;
        cash_assets.clear();

        Ok(())
    }

    fn get_price(&self, date: Date) -> GenericResult<Cash> {
        let mut nearest = Vec::new();

        if let Some((&price_date, &price)) = self.quotes.range(..=date).last() {
            if price_date >= self.spec.id.exchange.min_last_working_day(date) {
                return Ok(price);
            }
            nearest.push(price_date);
        }

        if let Some((&price_date, _price)) = self.quotes.range(date..).next() {
            nearest.push(price_date);
        }

        if nearest.is_empty() {
            return Err!("There are no historical quotes for {}", self.spec.id.symbol);
        }

        Err!(
            "There are no historical quotes for {} at {}. The nearest quotes we have are at {}",
            self.spec.id.symbol, formatting::format_date(date),
            nearest.into_iter().map(formatting::format_date).join(" and "),
        )
    }

    fn get_value(&self, date: Date) -> GenericResult<Cash> {
        Ok(self.get_price(date)? * self.quantity)
    }
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

struct BacktestingResult {
    date: Date,
    net_value: Decimal,
    performance: Option<Decimal>,
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
    benchmark: &Benchmark, method: BenchmarkPerformanceType, currency: &str, results: &[BacktestingResult],
) -> GenericResult<Vec<DailyTimeSeries>> {
    let time_series = |name: &str| {
        let mut time_series = DailyTimeSeries::new(&format!("{}_{name}", metrics::NAMESPACE))
            .with_label(metrics::INSTRUMENT_LABEL, &benchmark.name)
            .with_label(metrics::TYPE_LABEL, method.into())
            .with_label(metrics::CURRENCY_LABEL, currency);

        if let Some(ref provider) = benchmark.provider {
            time_series = time_series.with_label(metrics::PROVIDER_LABEL, provider);
        }

        time_series
    };

    let mut net_value_time_series = time_series(metrics::BACKTESTING_NET_VALUE_NAME);
    let mut performance_time_series = time_series(metrics::BACKTESTING_PERFORMANCE_NAME);

    for result in results {
        let net_value = result.net_value.to_f64().ok_or_else(|| format!(
            "Got an invalid result: {}", result.net_value))?;
        net_value_time_series.add_value(result.date, net_value);

        if let Some(performance) = result.performance {
            let performance = performance.to_f64().ok_or_else(|| format!(
                "Got an invalid performance: {}", performance))?;
            performance_time_series.add_value(result.date, performance);
        }
    }

    let mut metrics = vec![net_value_time_series];
    if !performance_time_series.is_empty() {
        metrics.push(performance_time_series);
    }

    Ok(metrics)
}