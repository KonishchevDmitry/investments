use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use itertools::Itertools;
use num_traits::Zero;

use crate::analysis::deposit::{Transaction, InterestPeriod};
use crate::core::{GenericResult, EmptyResult};
use crate::formatting;
use crate::time::{Date, DateOptTime};
use crate::types::Decimal;

pub struct InstrumentDepositView {
    symbol: String,
    pub name: Option<String>,
    trades: BTreeMap<Date, HashMap<String, Decimal>>,
    pub transactions: Vec<Transaction>,
    pub interest_periods: Vec<InterestPeriod>,
    pub closed: bool,
}

impl InstrumentDepositView {
    pub fn new(symbol: &str) -> InstrumentDepositView {
        InstrumentDepositView {
            symbol: symbol.to_owned(),
            name: None,
            trades: BTreeMap::new(),
            transactions: Vec::new(),
            interest_periods: Vec::new(),
            closed: true,
        }
    }

    pub fn trade(&mut self, portfolio_id: &str, symbol: &str, time: DateOptTime, quantity: Decimal) {
        // We should handle each portfolio separately to work properly with stock splits (different
        // portfolios with different open position periods may have different stock split
        // information and as a consequence - different quantity multipliers.
        let instrument_id = format!("{}:{}", portfolio_id, symbol);

        let position = self.trades.entry(time.date).or_default()
            .entry(instrument_id).or_default();

        *position += quantity;
    }

    pub fn transaction(&mut self, time: DateOptTime, amount: Decimal) {
        // Some assets can be acquired for free due to corporate actions or other non-trading
        // operations.
        if !amount.is_zero() {
            self.transactions.push(Transaction::new(time.date, amount))
        }
    }

    pub fn calculate_open_position_periods(&mut self) -> EmptyResult {
        if self.trades.is_empty() {
            return Err!("Got an unexpected transaction for {} which has no trades", self.symbol)
        }

        let mut open_position = None;
        let mut interest_periods = Vec::<InterestPeriod>::new();

        for (&date, trades) in &self.trades {
            let current = open_position.get_or_insert_with(||
                OpenPosition::new(date));

            if current.trade(date, trades)? {
                continue;
            }

            let open_date = current.open_date;
            let close_date = if date == open_date {
                date.succ_opt().unwrap()
            } else {
                date
            };

            match interest_periods.last_mut() {
                Some(ref mut period) if period.end >= open_date => {
                    assert_eq!(period.end, open_date);
                    assert!(period.end < close_date);
                    period.end = close_date;
                },
                _ => interest_periods.push(InterestPeriod::new(open_date, close_date)),
            };

            open_position = None;
        }

        if let Some(open_position) = open_position {
            return Err!(
                "The portfolio contains unsold stocks when sellout simulation is expected: {}",
                open_position.symbols.keys().join(", "));
        }

        assert!(!interest_periods.is_empty());
        self.interest_periods = interest_periods;

        Ok(())
    }
}

// Represents a logical opened position consisting from a few real symbols
struct OpenPosition<'a> {
    open_date: Date,
    symbols: HashMap<&'a str, Decimal>,
}

impl<'a> OpenPosition<'a> {
    fn new(open_date: Date) -> OpenPosition<'a> {
        OpenPosition { open_date, symbols: HashMap::new() }
    }

    fn trade(&mut self, date: Date, trades: &'a HashMap<String, Decimal>) -> GenericResult<bool> {
        for (symbol, quantity) in trades {
            let current = self.symbols.entry(symbol).or_default();
            *current += quantity;

            match (*current).cmp(&Decimal::zero()) {
                Ordering::Greater => {},
                Ordering::Equal => {
                    self.symbols.remove(symbol.as_str());
                },
                Ordering::Less => {
                    return Err!(
                        "Error while processing {} sell operations: Got a negative balance on {}",
                        symbol, formatting::format_date(date));
                }
            };
        }

        Ok(!self.symbols.is_empty())
    }
}