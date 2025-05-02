use std::collections::BTreeMap;
use std::ops::Bound;

use easy_logging::GlobalContext;
use itertools::Itertools;
use log::debug;
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
use crate::quotes::{Quotes, QuotesRc, QuoteQuery, HistoricalQuotes};
use crate::time::{self, Date, Period};
use crate::types::{Decimal, TradeType};

pub fn backtest(
    currency: &str, benchmarks: &[Benchmark], statements: &[BrokerStatement],
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

    let interest_periods = [InterestPeriod::new(start_date, time::today())];
    let days = portfolio_performance::get_total_activity_duration(&interest_periods);

    let mut results = BacktestingResultsTable::new();
    let format_interest = |interest| format!("{}%", interest);

    {
        let _logging_context = GlobalContext::new("Portfolio");

        let portfolio_interest = deposit_performance::compare_instrument_to_bank_deposit(
            "portfolio", currency, &transactions, &interest_periods, net_value.amount)?;

        results.add_row(BacktestingResults {
            benchmark: s!("Portfolio"),
            result: Cell::new_round_decimal(net_value.amount),
            interest: portfolio_interest.map(format_interest),
        });
    }

    for benchmark in benchmarks {
        let _logging_context = GlobalContext::new(&benchmark.name);

        let result = benchmark.backtest(currency, &cash_flows, converter.clone(), quotes.clone()).map_err(|e| format!(
            "Failed to backtest the portfolio against {} benchmark: {e}", benchmark.name))?;

        let interest = deposit_performance::compare_instrument_to_bank_deposit(
            &benchmark.name, currency, &transactions, &interest_periods, result.amount)?;

        results.add_row(BacktestingResults {
            benchmark: benchmark.name.clone(),
            result: Cell::new_round_decimal(result.amount),
            interest: interest.map(format_interest),
        });
    }

    results.print(&format!("Backtesting results ({})", formatting::format_days(days)));
    Ok(())
}

pub struct Benchmark {
    pub name: String,
    instruments: BTreeMap<Date, BenchmarkInstrument>
}

impl Benchmark {
    // FIXME(konishchev): Provider name for metric labels
    pub fn new(name: &str, instrument: BenchmarkInstrument) -> Benchmark {
        Benchmark {
            name: name.to_owned(),
            instruments: btreemap!{
                Date::MIN => instrument,
            }
        }
    }

    pub fn then(self, date: Date, instrument: BenchmarkInstrument) -> GenericResult<Self> {
        self.then_transition(date, instrument)
    }

    pub fn then_rename(self, date: Date, mut instrument: BenchmarkInstrument) -> GenericResult<Self> {
        instrument.renamed_from = Some(self.instruments.last_key_value().unwrap().1.id.clone());
        self.then_transition(date, instrument)
    }

    // FIXME(konishchev): Metrics generation
    pub fn backtest(&self, currency: &str, cash_flows: &[CashAssets], converter: CurrencyConverterRc, quotes: QuotesRc) -> GenericResult<Cash> {
        debug!("Backtesting {}...", self.name);

        let start_date = cash_flows.first().ok_or(
            "An attempt to backtest an empty portfolio (without cash flows)")?.date;
        let end_date = cash_flows.last().unwrap().date;

        let (transition_date, instrument) = self.select_instrument(start_date, end_date, &quotes, true)?;
        assert_eq!(transition_date, start_date);

        Backtester {
            currency, quotes, converter,

            benchmark: self,
            cash_flows,

            date: start_date,
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

        // FIXME(konishchev): Historical quotes caching
        let candles = quotes.get_historical(instrument.id.exchange, &instrument.id.symbol, quotes_period)?;

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

    benchmark: &'a Benchmark,
    cash_flows: &'a [CashAssets],

    date: Date,
    cash_assets: MultiCurrencyCashAccount,
    instrument: CurrentBenchmarkInstrument<'a>,
}

impl Backtester<'_> {
    // FIXME(konishchev): Daily statistics
    fn backtest(mut self) -> GenericResult<Cash> {
        for cash_flow in self.cash_flows {
            debug!("Backtesting cash flow: {}: {}...", formatting::format_date(cash_flow.date), cash_flow.cash);
            self.process_to(cash_flow.date)?;
            self.cash_assets.deposit(cash_flow.cash);
            self.instrument.process_cash_flow(cash_flow.date, &mut self.cash_assets, self.converter.clone(), false)?;
        }

        self.process_to(time::today())?;
        let instrument = self.instrument.spec.id.clone();
        let price = self.quotes.get(QuoteQuery::Stock(instrument.symbol, vec![instrument.exchange]))?;

        let net_value =
            self.converter.real_time_convert_to(price * self.instrument.quantity, self.currency)? +
            self.cash_assets.total_assets_real_time(self.currency, &self.converter)?;

        Ok(Cash::new(self.currency, net_value))
    }

    fn process_to(&mut self, date: Date) -> EmptyResult {
        assert!(date >= self.date);
        self.date = date;

        let Some(transition_date) = self.instrument.until else {
            return Ok(());
        };

        if date < transition_date {
            return Ok(());
        }

        let (transition_date, mut new_instrument) = self.benchmark.select_instrument(
            date, self.cash_flows.last().unwrap().date, &self.quotes, false)?;

        match new_instrument.spec.renamed_from.as_ref() {
            Some(renamed_from) if *renamed_from == self.instrument.spec.id => {
                new_instrument.quantity = self.instrument.quantity;
            },
            _ => {
                let net_value = self.instrument.get_price(transition_date)? * self.instrument.quantity;
                self.cash_assets.deposit(net_value);
                new_instrument.process_cash_flow(transition_date, &mut self.cash_assets, self.converter.clone(), true)?;
            },
        }

        debug!("Converted {} {} -> {} {} at {}.",
            self.instrument.quantity, self.instrument.spec.id.symbol,
            new_instrument.quantity, new_instrument.spec.id.symbol,
            formatting::format_date(transition_date));

        self.instrument = new_instrument;
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
    commission_spec: CommissionSpec,
    renamed_from: Option<BenchmarkInstrumentId>,
}

impl BenchmarkInstrument {
    pub fn new(symbol: &str, exchange: Exchange, commission_spec: CommissionSpec) -> BenchmarkInstrument {
        BenchmarkInstrument {
            id: BenchmarkInstrumentId {
                symbol: symbol.to_owned(),
                exchange,
            },
            commission_spec,
            renamed_from: None,
        }
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
}

#[derive(StaticTable)]
#[table(name="BacktestingResultsTable")]
struct BacktestingResults {
    #[column(name="Benchmark")]
    benchmark: String,
    #[column(name="Result")]
    result: Cell,
    #[column(name="Interest", align="right")]
    interest: Option<String>,
}