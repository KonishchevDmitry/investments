use std::collections::BTreeMap;
use std::ops::Bound;

use itertools::Itertools;
use log::debug;

use crate::analysis::deposit::Transaction;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets, MultiCurrencyCashAccount};
use crate::currency::converter::{CurrencyConverter, CurrencyConverterRc};
use crate::exchanges::Exchange;
use crate::formatting;
use crate::quotes::{Quotes, QuotesRc, HistoricalQuotes};
use crate::time::{Date, Period};
use crate::types::Decimal;

use super::BenchmarkPerformanceType;
use super::config::{BenchmarkConfig, TransitionType};
use super::benchmark::{Benchmark, BacktestingResult};

pub struct StockBenchmark {
    name: String,
    provider: Option<String>,
    instruments: BTreeMap<Date, Instrument>,
    converter: CurrencyConverterRc,
    quotes: QuotesRc,
}

impl StockBenchmark {
    pub fn new(config: &BenchmarkConfig, converter: CurrencyConverterRc, quotes: QuotesRc) -> GenericResult<Self> {
        let mut benchmark = StockBenchmark {
            name: config.name.clone(),
            provider: config.provider.clone(),
            instruments: btreemap!{
                Date::MIN => Instrument::new(&config.symbol, config.exchange, config.aliases.clone()),
            },
            converter, quotes,
        };

        for (&date, transition) in &config.transitions {
            let exchange = transition.exchange.unwrap_or(config.exchange);
            let mut instrument = Instrument::new(&transition.symbol, exchange, transition.aliases.clone());

            match transition.transition_type.unwrap_or(TransitionType::Convert) {
                TransitionType::Convert => {},
                TransitionType::Rename => {
                    let last_instrument = benchmark.instruments.last_key_value().unwrap().1;
                    instrument.renamed_from = Some(last_instrument.id.clone());
                },
            }

            assert!(benchmark.instruments.insert(date, instrument).is_none());
        }

        Ok(benchmark)
    }

    fn select_instrument(&self, date: Date, today: Date, quotes: &Quotes, first: bool) -> GenericResult<(Date, CurrentInstrument)> {
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
            std::cmp::max(transition_date, until.unwrap_or(today)),
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

        Ok((transition_date, CurrentInstrument {
            spec: instrument,
            quotes: candles,
            until,
            quantity: dec!(0),
        }))
    }
}

impl Benchmark for StockBenchmark {
    fn name(&self) -> String {
        match self.provider.as_ref() {
            Some(provider) => format!("{} ({provider})", self.name),
            None => self.name.clone(),
        }
    }

    fn provider(&self) -> Option<String> {
        self.provider.clone()
    }

    fn backtest(
        &self, method: BenchmarkPerformanceType, currency: &str, cash_flows: &[CashAssets], today: Date, full: Option<Date>,
    ) -> GenericResult<Vec<BacktestingResult>> {
        let start_date = cash_flows.first().unwrap().date;

        let (transition_date, instrument) = self.select_instrument(start_date, today, &self.quotes, true)?;
        assert_eq!(transition_date, start_date);

        Backtester {
            method, currency,
            quotes: self.quotes.clone(),
            converter: self.converter.clone(),

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
}

struct Backtester<'a> {
    method: BenchmarkPerformanceType,
    currency: &'a str,
    quotes: QuotesRc,
    converter: CurrencyConverterRc,

    benchmark: &'a StockBenchmark,
    cash_flows: &'a [CashAssets],
    transactions: Vec<Transaction>,
    results: Vec<BacktestingResult>,
    full: Option<Date>,

    date: Date,
    today: Date,

    cash_assets: MultiCurrencyCashAccount,
    instrument: CurrentInstrument<'a>,
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
            if self.full.is_some() {
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
                self.date, self.today, &self.quotes, false)?;

            if self.full.is_some() {
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

        // Intentionally don't try to use real time quotes for today's date because real time and historical quotes
        // providers give access to different stocks.
        let net_value = {
            let mut net_value = MultiCurrencyCashAccount::new();
            net_value.add(&self.cash_assets);
            net_value.deposit(self.instrument.get_value(self.date)?);
            net_value.total_cash_assets(self.date, self.currency, &self.converter)?
        };

        let performance_from = std::cmp::min(self.full.unwrap_or(self.today), self.today);

        self.results.push(BacktestingResult::calculate(
            &self.benchmark.name(), self.date, net_value,
            self.method, &self.transactions, self.date >= performance_from)?);

        self.date = self.date.succ_opt().unwrap();

        Ok(())
    }
}

#[derive(Clone, PartialEq)]
struct InstrumentId {
    symbol: String,
    exchange: Exchange,
}

struct Instrument {
    id: InstrumentId,
    renamed_from: Option<InstrumentId>,

    // Historical API handle instrument renames differently:
    // * With MOEX API you need to request quotes for different symbols depending on the period.
    // * T-Bank API forgets all previous instrument symbols and returns all quotes by its current symbol.
    //
    // This field is used to support both provider types.
    aliases: Vec<String>,
}

impl Instrument {
    fn new(symbol: &str, exchange: Exchange, aliases: Vec<String>) -> Self {
        Instrument {
            id: InstrumentId {
                symbol: symbol.to_owned(),
                exchange,
            },
            renamed_from: None,
            aliases,
        }
    }
}

struct CurrentInstrument<'a> {
    spec: &'a Instrument,
    quotes: HistoricalQuotes,
    until: Option<Date>,
    quantity: Decimal,
}

impl CurrentInstrument<'_> {
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