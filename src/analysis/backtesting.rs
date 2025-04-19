use std::collections::BTreeMap;

use num_traits::Zero;

use crate::core::GenericResult;
use crate::currency::{Cash, CashAssets};
use crate::currency::converter::CurrencyConverter;
use crate::exchanges::Exchange;
use crate::quotes::{QuoteQuery, Quotes};
use crate::time::{self, Date, Period};
use crate::types::Decimal;

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

    pub fn backtest(&self, currency: &str, cash_flows: &[CashAssets], converter: &CurrencyConverter, quotes: &Quotes) -> GenericResult<Cash> {
        let benchmark = self.instruments.first_key_value().unwrap().1;

        let candles = quotes.get_historical(benchmark.exchange, &benchmark.symbol, Period::new(cash_flows.first().unwrap().date, cash_flows.last().unwrap().date)?)?;
        let mut current_quantity = Decimal::zero();

        for cash_flow in cash_flows {
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
}

impl BenchmarkInstrument {
    pub fn new(symbol: &str, exchange: Exchange) -> BenchmarkInstrument {
        BenchmarkInstrument {
            symbol: symbol.to_owned(),
            exchange,
        }
    }
}