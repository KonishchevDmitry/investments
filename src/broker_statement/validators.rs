use std::collections::{HashMap, hash_map::Entry};

use crate::core::EmptyResult;
use crate::formatting;
use crate::time::{DateOptTime, Date};

use super::{StockBuy, StockSell};

pub struct DateValidator {
    pub min_date: Date,
    pub max_date: Date,
}

impl DateValidator {
    pub fn new(min_date: Date, max_date: Date) -> DateValidator {
        DateValidator {min_date, max_date}
    }

    pub fn sort_and_validate<T, D>(
        &self, name: &str, objects: &mut [T], get_date: fn(&T) -> D,
    ) -> EmptyResult where D: Into<DateOptTime> {
        objects.sort_by_key(|object| get_date(object).into());
        self.validate(name, objects, get_date)?;
        Ok(())
    }

    pub fn validate<T, D>(
        &self, name: &str, objects: &[T], get_date: fn(&T) -> D,
    ) -> EmptyResult where D: Into<DateOptTime> {
        if objects.is_empty() {
            return Ok(());
        }

        let first_date = get_date(objects.first().unwrap()).into().date;
        let last_date = get_date(objects.last().unwrap()).into().date;

        if first_date < self.min_date {
            return Err!("Got {} outside of statement period: {}",
                        name, formatting::format_date(first_date));
        }

        if last_date > self.max_date {
            return Err!("Got {} outside of statement period: {}",
                        name, formatting::format_date(last_date));
        }

        Ok(())
    }
}

pub struct TradeInfo<'a> {
    symbol: &'a str,
    conclusion_date: Date,
    execution_date: Date,
    margin: bool,
}

pub trait StockTrade {
    fn info(&self) -> TradeInfo;
}

impl StockTrade for StockBuy {
    fn info(&self) -> TradeInfo {
        TradeInfo {
            symbol: &self.symbol,
            conclusion_date: self.conclusion_date,
            execution_date: self.execution_date,
            margin: self.margin,
        }
    }
}

impl StockTrade for StockSell {
    fn info(&self) -> TradeInfo {
        TradeInfo {
            symbol: &self.symbol,
            conclusion_date: self.conclusion_date,
            execution_date: self.execution_date,
            margin: self.margin,
        }
    }
}

pub fn sort_and_validate_trades<T: StockTrade>(name: &str, trades: &mut [T]) -> EmptyResult {
    // Checking trades order to be sure that we won't be surprised during FIFO calculation.
    //
    // Stocks may have different settlement duration for example due to corporate actions, so check
    // the order only for trades of the same stock.

    trades.sort_by_key(|trade| {
        let trade = trade.info();
        (trade.conclusion_date, trade.execution_date)
    });

    let mut symbols: HashMap<&str, TradeInfo> = HashMap::new();

    for trade in trades {
        let trade = trade.info();

        match symbols.entry(trade.symbol) {
            Entry::Occupied(mut entry) => {
                let prev = entry.get_mut();

                if trade.execution_date < prev.execution_date && !trade.margin {
                    return Err!(
                        "Got an unexpected execution order of {} trades for {}: {} -> {}, {} -> {}",
                        name, trade.symbol,
                        formatting::format_date(prev.conclusion_date),
                        formatting::format_date(prev.execution_date),
                        formatting::format_date(trade.conclusion_date),
                        formatting::format_date(trade.execution_date));
                }

                entry.insert(trade);
            },
            Entry::Vacant(entry) => {
                entry.insert(trade);
            },
        };
    }

    Ok(())
}