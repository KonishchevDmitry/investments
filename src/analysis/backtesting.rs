use std::collections::BTreeMap;

use itertools::Itertools;
use num_traits::Zero;
use static_table_derive::StaticTable;

use crate::broker_statement::BrokerStatement;
use crate::commissions::CommissionSpec;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::currency::converter::CurrencyConverterRc;
use crate::exchanges::Exchange;
use crate::formatting::table::Cell;
use crate::quotes::{QuoteQuery, Quotes};
use crate::time::{self, Date, Period};
use crate::types::Decimal;

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

    let mut results = BacktestingResultsTable::new();

    results.add_row(BacktestingResults {
        benchmark: s!("Portfolio"),
        result: Cell::new_round_decimal(net_value.amount),
        interest: None, // FIXME(konishchev): Implement
        // interest: self.interest.map(|interest| format!("{}%", interest)),
    });

    for benchmark in benchmarks {
        let result = benchmark.backtest(currency, &cash_flows, converter.clone(), quotes)?;

        results.add_row(BacktestingResults {
            benchmark: benchmark.name.clone(),
            result: Cell::new_round_decimal(result.amount),
            interest: None, // FIXME(konishchev): Implement
            // interest: self.interest.map(|interest| format!("{}%", interest)),
        });
    }

    results.print("Backtesting results");
    Ok(())
}

pub struct Benchmark {
    name: String,
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

    // FIXME(konishchev): Implement
    pub fn backtest(&self, currency: &str, cash_flows: &[CashAssets], converter: CurrencyConverterRc, quotes: &Quotes) -> GenericResult<Cash> {
        let benchmark = self.instruments.first_key_value().unwrap().1;

        let candles = quotes.get_historical(benchmark.exchange, &benchmark.symbol, Period::new(cash_flows.first().unwrap().date, cash_flows.last().unwrap().date)?)?;
        let mut current_quantity = Decimal::zero();

        for cash_flow in cash_flows {
            // let mut commission_calc = CommissionCalc::new(
            //     converter.clone(), statement.broker.commission_spec.clone(), net_value)?;
            // let commission = commission_calc.add_trade(
            //     conclusion_time.date, TradeType::Sell, quantity, price)?;
            //     let mut total = MultiCurrencyCashAccount::new();

            //     for commissions in commission_calc.calculate()?.values() {
            //         for commission in commissions.iter() {
            //             self.assets.cash.withdraw(commission);
            //             total.deposit(commission);
            //         }
            //     }

            let candle_range = candles.range(..=cash_flow.date);
            let candle = candle_range.last().unwrap();

            let price = *candle.1;
            let amount = converter.convert_to_cash(cash_flow.date, cash_flow.cash, price.currency)?;

            let quantity = amount / price;

            if cash_flow.cash.is_negative() {
                current_quantity -= quantity;
            } else {
                current_quantity += quantity;
            }
        }

        let price = quotes.get(QuoteQuery::Stock(benchmark.symbol.clone(), vec![benchmark.exchange]))?;
        let net_assets = converter.convert_to_cash_rounding(time::today(), price * current_quantity, currency)?;

        Ok(net_assets)
    }
}

pub struct BenchmarkInstrument {
    symbol: String,
    exchange: Exchange,
    #[allow(dead_code)] // FIXME(konishchev): Drop it
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