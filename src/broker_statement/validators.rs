use crate::core::EmptyResult;
use crate::formatting;
use crate::types::Date;

use super::{StockBuy, StockSell};

pub struct DateValidator {
    min_date: Date,
    max_date: Date,
}

impl DateValidator {
    pub fn new(min_date: Date, max_date: Date) -> DateValidator {
        DateValidator {min_date, max_date}
    }

    pub fn sort_and_validate<T>(&self, name: &str, objects: &mut [T], get_date: fn(&T) -> Date) -> EmptyResult {
        if !objects.is_empty() {
            objects.sort_by_key(get_date);
            self.validate(name, objects, get_date)?;
        }

        Ok(())
    }

    pub fn validate<T>(&self, name: &str, objects: &[T], get_date: fn(&T) -> Date) -> EmptyResult {
        let first_date = get_date(objects.first().unwrap());
        let last_date = get_date(objects.first().unwrap());

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

pub fn validate_trades<T: StockTrade>(name: &str, trades: &mut [T]) -> EmptyResult {
    trades.sort_by_key(|trade| {
        let trade = trade.info();
        (trade.conclusion_date, trade.execution_date)
    });

    let mut prev = None;

    // FIXME(konishchev): Relative order?
    for trade in trades {
        let trade = trade.info();

        if let Some((prev_symbol, prev_conclusion_date, prev_execution_date)) = prev {
            if trade.execution_date < prev_execution_date && !trade.margin {
                return Err!(
                    "Got an unexpected execution order of {} trades: {} -> {} {}, {} -> {} {}", name,
                    formatting::format_date(prev_conclusion_date), formatting::format_date(prev_execution_date), prev_symbol,
                    formatting::format_date(trade.conclusion_date), formatting::format_date(trade.execution_date), trade.symbol);
            }
        }
        prev.replace((trade.symbol, trade.conclusion_date, trade.execution_date));
    }

    Ok(())
}