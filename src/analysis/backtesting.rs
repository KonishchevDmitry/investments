use std::collections::BTreeMap;
use std::ops::Bound;

use easy_logging::GlobalContext;
use itertools::Itertools;
use log::debug;
use num_traits::ToPrimitive;
use static_table_derive::StaticTable;

use crate::analysis::deposit_emulator::{InterestPeriod, Transaction};
use crate::analysis::deposit_performance;
use crate::analysis::portfolio_performance;
use crate::broker_statement::BrokerStatement;
use crate::commissions::{CommissionCalc, CommissionSpec};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverterRc;
use crate::exchanges::Exchange;
use crate::formatting;
use crate::formatting::table::Cell;
use crate::metrics::{self, backfilling::DailyTimeSeries};
use crate::quotes::{Quotes, QuotesRc, QuoteQuery, HistoricalQuotes};
use crate::time::{self, Date, Period};
use crate::types::{Decimal, TradeType};

pub fn backtest(
    currency: &str, benchmarks: &[Benchmark], statements: &[BrokerStatement], mut metrics: Option<&mut Vec<DailyTimeSeries>>,
    converter: CurrencyConverterRc, quotes: QuotesRc,
) -> EmptyResult {
    let mut cash_flows = BTreeMap::new();
    let mut net_value = Cash::zero(currency);

    for statement in statements {
        for cash_flow in &statement.deposits_and_withdrawals {
            cash_flows.entry((cash_flow.date, cash_flow.cash.currency))
                .and_modify(|result| *result += cash_flow.cash.amount)
                .or_insert(cash_flow.cash.amount);
        }
        net_value += statement.net_value(&converter, &quotes, currency, true)?;
    }

    let cash_flows = cash_flows.into_iter().filter_map(|((date, currency), amount)| {
        if amount.is_zero() {
            None
        } else {
            Some(CashAssets::new(date, currency, amount))
        }
    }).collect_vec();

    let transactions = {
        let mut transactions = BTreeMap::new();

        for cash_flow in &cash_flows {
            let amount = converter.convert_to(cash_flow.date, cash_flow.cash, currency)?;
            transactions.entry(cash_flow.date)
                .and_modify(|result| *result += amount)
                .or_insert(amount);
        }

        transactions.into_iter().map(|(date, amount)| {
            Transaction::new(date, amount)
        }).collect_vec()
    };

    let start_date = transactions.first().ok_or(
        "An attempt to backtest an empty portfolio (without cash flows)")?.date;

    let today = time::today();
    if start_date == today {
        return Err!("An attempt to backtest portfolio which was created today");
    } else if today < transactions.last().unwrap().date {
        return Err!("The portfolio contains future cash flows");
    }

    let interest_periods = [InterestPeriod::new(start_date, today)];
    let days = portfolio_performance::get_total_activity_duration(&interest_periods);

    let mut table = BacktestingResultsTable::new();
    let format_interest = |interest| format!("{}%", interest);

    {
        let _logging_context = GlobalContext::new("Portfolio");

        let portfolio_interest = deposit_performance::compare_instrument_to_bank_deposit(
            "portfolio", currency, &transactions, &interest_periods, net_value.amount)?;

        table.add_row(BacktestingResultsTableRow {
            benchmark: s!("Portfolio"),
            result: Cell::new_round_decimal(net_value.amount),
            interest: portfolio_interest.map(format_interest),
        });
    }

    for benchmark in benchmarks {
        let _logging_context = GlobalContext::new(&benchmark.name());

        let results = benchmark.backtest(
            currency, &cash_flows, metrics.is_some(), today, converter.clone(), quotes.clone(),
        ).map_err(|e| format!("Failed to backtest the portfolio against {} benchmark: {e}", benchmark.name()))?;

        let current = results.last().unwrap();
        assert_eq!(current.date, today);

        table.add_row(BacktestingResultsTableRow {
            benchmark: benchmark.name(),
            result: Cell::new_round_decimal(current.result),
            interest: current.interest.map(format_interest),
        });

        let Some(metrics) = metrics.as_mut() else {
            continue;
        };

        let namespace = format!("{}_backtesting", metrics::NAMESPACE);

        let time_series = |name: &str| {
            let mut time_series = DailyTimeSeries::new(&format!("{namespace}_{name}"))
                .with_label("currency", currency)
                .with_label("instrument", &benchmark.name);

            if let Some(ref provider) = benchmark.provider {
                time_series = time_series.with_label("provider", provider);
            }

            time_series
        };

        let mut net_value_time_series = time_series("net_value");
        let mut performance_time_series = time_series("performance");

        for result in results {
            let net_value = result.result.to_f64().ok_or_else(|| format!(
                "Got an invalid result for {}: {}", benchmark.name(), result.result))?;
            net_value_time_series.add_value(result.date, net_value);

            if let Some(interest) = result.interest {
                let interest = interest.to_f64().ok_or_else(|| format!(
                    "Got an invalid performance for {}: {}", benchmark.name(), interest))?;
                performance_time_series.add_value(result.date, interest);
            }
        }

        metrics.push(net_value_time_series);
        if !performance_time_series.is_empty() {
            metrics.push(performance_time_series);
        }
    }

    table.print(&format!("Backtesting results ({currency}, {})", formatting::format_days(days)));
    Ok(())
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
        &self, currency: &str, cash_flows: &[CashAssets], full: bool,
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
            currency, quotes, converter, quotes_limit,

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
            self.instrument.process_cash_flow(cash_flow.date, &mut self.cash_assets, self.converter.clone(), false)?;
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
                    new_instrument.process_cash_flow(transition_date, &mut self.cash_assets, self.converter.clone(), true)?;
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
            result: net_value,
            interest: None,
        };

        let start_date = self.transactions.first().unwrap().date;
        if (self.date - start_date).num_days() > 0 { // FIXME(konishchev): Alter the value
            let name = format!("{} @ {}", self.benchmark.name(), formatting::format_date(self.date));
            let interest_periods = [InterestPeriod::new(start_date, self.date)];

            result.interest = deposit_performance::compare_instrument_to_bank_deposit(
                &name, self.currency, &self.transactions, &interest_periods, net_value)?;
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
    commission_spec: CommissionSpec,
    renamed_from: Option<BenchmarkInstrumentId>,
}

impl BenchmarkInstrument {
    pub fn new(symbol: &str, exchange: Exchange, commission_spec: CommissionSpec) -> Self {
        BenchmarkInstrument {
            id: BenchmarkInstrumentId {
                symbol: symbol.to_owned(),
                exchange,
            },
            aliases: Vec::new(),
            commission_spec,
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
        &mut self, date: Date, cash_assets: &mut MultiCurrencyCashAccount, converter: CurrencyConverterRc,
        commission_free: bool,
    ) -> EmptyResult {
        let price = self.get_price(date)?;
        let cash = cash_assets.total_cash_assets(date, price.currency, &converter)?;
        let net_value = price * self.quantity + cash;

        let change = cash / price;
        if change.is_zero() {
            return Ok(());
        }

        cash_assets.clear();
        self.quantity += change;

        if commission_free {
            return Ok(());
        }

        let trade_type = if change.is_sign_positive() {
            TradeType::Buy
        } else {
            TradeType::Sell
        };

        let commission_spec = self.spec.commission_spec.clone();
        if !commission_spec.is_simple_percent() {
            return Err!(concat!(
                "Current backtesting logic doesn't support complex commissions which require trade aggregation or ",
                "have non-percent fees"
            ));
        }

        let mut commission_calc = CommissionCalc::new(converter.clone(), commission_spec, net_value)?;

        let commission = commission_calc.add_trade(date, trade_type, change.abs(), price)?;
        cash_assets.withdraw(commission);

        for commissions in commission_calc.calculate()?.values() {
            for commission in commissions.iter() {
                cash_assets.withdraw(commission);
            }
        }

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

struct BacktestingResult {
    date: Date,
    result: Decimal,
    interest: Option<Decimal>,
}

#[derive(StaticTable)]
#[table(name="BacktestingResultsTable")]
struct BacktestingResultsTableRow {
    #[column(name="Benchmark")]
    benchmark: String,
    #[column(name="Result")]
    result: Cell,
    #[column(name="Interest", align="right")]
    interest: Option<String>,
}