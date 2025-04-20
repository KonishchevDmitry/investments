use std::collections::BTreeMap;
use std::ops::Bound;


use itertools::Itertools;
use log::trace;
use static_table_derive::StaticTable;

use crate::analysis::deposit_emulator::{InterestPeriod, Transaction};
use crate::analysis::deposit_performance;
use crate::broker_statement::BrokerStatement;
use crate::commissions::{CommissionCalc, CommissionSpec};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverterRc;
use crate::exchanges::Exchange;
use crate::formatting;
use crate::formatting::table::Cell;
use crate::quotes::{QuoteQuery, Quotes, HistoricalQuotes};
use crate::time::{self, Date, Period};
use crate::types::{Decimal, TradeType};

pub fn backtest(
    currency: &str, benchmarks: &[Benchmark], statements: &[BrokerStatement],
    converter: CurrencyConverterRc, quotes: &Quotes,
) -> EmptyResult {
    let mut cash_flows = BTreeMap::new();
    let mut net_value = Cash::zero(currency);

    for statement in statements {
        for cash_flow in &statement.deposits_and_withdrawals {
            cash_flows.entry((cash_flow.date, cash_flow.cash.currency))
                .and_modify(|result| *result += cash_flow.cash.amount)
                .or_insert(cash_flow.cash.amount);
        }

        net_value += statement.net_value(&converter, quotes, currency, true)?;
    }

    let cash_flows = cash_flows.into_iter()
        .filter_map(|((date, currency), amount)| {
            if amount.is_zero() {
                None
            } else {
                Some(CashAssets::new(date, currency, amount))
            }
        })
        .collect_vec();

    let transactions = cash_flows.iter().map(|cash_flow| {
        Transaction::new(cash_flow.date, cash_flow.cash.amount)
    }).collect_vec();

    let start_date = transactions.first().ok_or(
        "An attempt to backtest an empty portfolio (without cash flows)")?.date;

    let interest_periods = [InterestPeriod::new(start_date, time::today())];
    let portfolio_interest = deposit_performance::compare_instrument_to_bank_deposit(
        "portfolio", currency, &transactions, &interest_periods, net_value.amount)?;

    let mut results = BacktestingResultsTable::new();
    let format_interest = |interest| format!("{}%", interest);

    results.add_row(BacktestingResults {
        benchmark: s!("Portfolio"),
        result: Cell::new_round_decimal(net_value.amount),
        interest: portfolio_interest.map(format_interest),
    });

    for benchmark in benchmarks {
        let result = benchmark.backtest(currency, &cash_flows, converter.clone(), quotes).map_err(|e| format!(
            "Failed to backtest the portfolio against {} benchmark: {e}", benchmark.name))?;

        let interest = deposit_performance::compare_instrument_to_bank_deposit(
            &benchmark.name, currency, &transactions, &interest_periods, result.amount)?;

        results.add_row(BacktestingResults {
            benchmark: benchmark.name.clone(),
            result: Cell::new_round_decimal(result.amount),
            interest: interest.map(format_interest),
        });
    }

    results.print("Backtesting results");
    Ok(())
}

pub struct Benchmark {
    pub name: String,
    instruments: BTreeMap<Date, BenchmarkInstrument>
}

impl Benchmark {
    pub fn new(name: &str, instrument: BenchmarkInstrument) -> Benchmark {
        Benchmark {
            name: name.to_owned(),
            instruments: btreemap!{
                Date::MIN => instrument,
            }
        }
    }

    pub fn backtest(&self, currency: &str, cash_flows: &[CashAssets], converter: CurrencyConverterRc, quotes: &Quotes) -> GenericResult<Cash> {
        let mut cash_assets = MultiCurrencyCashAccount::new();
        let mut current_instrument: Option<CurrentBenchmarkInstrument<'_>> = None;

        let last_cash_flow_date = cash_flows.last().ok_or(
            "An attempt to backtest an empty portfolio (without cash flows)")?.date;

        for cash_flow in cash_flows {
            trace!("Backtesting cash flow: {}: {}...", formatting::format_date(cash_flow.date), cash_flow.cash);
            assert!(cash_flow.date <= last_cash_flow_date);

            let instrument = match current_instrument.as_mut() {
                Some(instrument) if instrument.until.is_none() || instrument.until.unwrap() > cash_flow.date => instrument,
                _ => {
                    let new_instrument = if let Some(old_instrument) = current_instrument.take() {
                        let conversion_date = old_instrument.until.unwrap();
                        assert!(conversion_date <= cash_flow.date);

                        let net_value = old_instrument.get_price(conversion_date)? * old_instrument.quantity;
                        cash_assets.deposit(net_value);

                        let mut new_instrument = self.select_instrument(conversion_date, last_cash_flow_date, quotes)?;
                        cash_assets = new_instrument.process_cash_flow(conversion_date, cash_assets, converter.clone(), true)?;

                        new_instrument
                    } else {
                        self.select_instrument(cash_flow.date, last_cash_flow_date, quotes)?
                    };

                    current_instrument.insert(new_instrument)
                },
            };

            cash_assets.deposit(cash_flow.cash);
            cash_assets = instrument.process_cash_flow(cash_flow.date, cash_assets, converter.clone(), false)?;
        }

        let instrument = current_instrument.unwrap();
        let price = quotes.get(QuoteQuery::Stock(instrument.spec.symbol.clone(), vec![instrument.spec.exchange]))?;

        let net_value =
            converter.real_time_convert_to(price * instrument.quantity, currency)? +
            cash_assets.total_assets_real_time(currency, &converter)?;

        Ok(Cash::new(currency, net_value))
    }

    fn select_instrument(&self, date: Date, last_cash_flow_date: Date, quotes: &Quotes) -> GenericResult<CurrentBenchmarkInstrument> {
        let Some((start_date, instrument)) = self.instruments.range(..date).last() else {
            return Err!("There is no benchmark instrument for {}", formatting::format_date(date));
        };

        trace!("Select new benchmark instrument: {}.", instrument.symbol);

        let until = self.instruments.range((Bound::Excluded(start_date), Bound::Unbounded)).next()
            .map(|(start_date, _instrument)| start_date).copied();

        let quotes_period = Period::new(
            instrument.exchange.min_last_working_day(date),
            std::cmp::min(
                last_cash_flow_date,
                until.unwrap_or(last_cash_flow_date),
            ),
        )?;

        let candles = quotes.get_historical(instrument.exchange, &instrument.symbol, quotes_period)?;

        Ok(CurrentBenchmarkInstrument {
            spec: instrument,
            quotes: candles,
            until,
            quantity: dec!(0),
        })
    }
}

pub struct BenchmarkInstrument {
    symbol: String,
    exchange: Exchange,
    commission_spec: CommissionSpec,
}

impl BenchmarkInstrument {
    pub fn new(symbol: &str, exchange: Exchange, commission_spec: CommissionSpec) -> BenchmarkInstrument {
        BenchmarkInstrument {
            symbol: symbol.to_owned(),
            exchange,
            commission_spec,
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
        &mut self, date: Date, mut cash_assets: MultiCurrencyCashAccount, converter: CurrencyConverterRc,
        commission_free: bool,
    ) -> GenericResult<MultiCurrencyCashAccount> {
        let price = self.get_price(date)?;
        let cash = cash_assets.total_cash_assets(date, price.currency, &converter)?;
        let net_value = price * self.quantity + cash;

        let change = cash / price;
        if change.is_zero() {
            return Ok(cash_assets);
        }

        cash_assets.clear();
        self.quantity += change;

        if !commission_free {
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
        }

        Ok(cash_assets)
    }

    fn get_price(&self, date: Date) -> GenericResult<Cash> {
        let mut nearest = Vec::new();

        if let Some((&price_date, &price)) = self.quotes.range(..=date).last() {
            if price_date >= self.spec.exchange.min_last_working_day(date) {
                return Ok(price);
            }
            nearest.push(price_date);
        }

        if let Some((&price_date, _price)) = self.quotes.range(date..).next() {
            nearest.push(price_date);
        }

        if nearest.is_empty() {
            return Err!("There are no historical quotes for {}", self.spec.symbol);
        }

        Err!(
            "There are no historical quotes for {} at {}. The nearest quotes we have are at {}",
            self.spec.symbol, formatting::format_date(date),
            nearest.into_iter().map(|date| formatting::format_date(date)).join(" and "),
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