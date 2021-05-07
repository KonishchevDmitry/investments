use std::collections::{HashMap, BTreeMap, hash_map, btree_map};
use std::ops::Bound;

use lazy_static::lazy_static;
use log::debug;
use num_traits::{ToPrimitive, Zero};
use regex::{self, Regex};
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::{EmptyResult, GenericResult};
use crate::formatting::format_date;
use crate::time::{Date, DateTime, DateOptTime, deserialize_date_opt_time};
use crate::types::Decimal;
use crate::util;

use super::BrokerStatement;
use super::trades::{StockBuy, StockSell, StockSellSource, PurchaseTotalCost};

#[derive(Deserialize, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CorporateAction {
    // Time when the corporate action has occurred. If time is not present, assuming:
    // * The changes are made at the end of the trading day.
    // * All trade operations from this day are assumed to be issued after the corporate action has
    //   occurred and are actually a part of the corporate action.
    #[serde(rename="date", deserialize_with = "deserialize_date_opt_time")]
    pub time: DateOptTime,

    // Report date from IB statements. Typically, it's T+1 date, so use it as trade execution date.
    #[serde(skip)]
    pub report_date: Option<Date>,

    pub symbol: String,
    #[serde(flatten)]
    pub action: CorporateActionType,
}

impl CorporateAction {
    fn execution_date(&self) -> Date {
        self.report_date.unwrap_or_else(|| self.time.date.succ())
    }
}

#[derive(Deserialize, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "type", rename_all="kebab-case")]
pub enum CorporateActionType {
    StockSplit {
        ratio: StockSplitRatio,

        #[serde(skip)]
        from_change: Option<Decimal>,

        #[serde(skip)]
        to_change: Option<Decimal>,
    },

    // See https://github.com/KonishchevDmitry/investments/issues/29 for details
    Rename {
        new_symbol: String,
    },

    // See https://github.com/KonishchevDmitry/investments/issues/20 for details
    #[serde(skip)]
    Spinoff {
        symbol: String,
        quantity: Decimal,
        currency: String,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct StockSplitRatio {
    pub from: u32,
    pub to: u32,
}

impl StockSplitRatio {
    pub fn new(from: u32, to: u32) -> StockSplitRatio {
        StockSplitRatio {from, to}
    }
}

impl<'de> Deserialize<'de> for StockSplitRatio {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let ratio: String = Deserialize::deserialize(deserializer)?;

        lazy_static! {
            static ref REGEX: Regex = Regex::new(r"^(?P<to>[1-9]\d*):(?P<from>[1-9]\d*)$").unwrap();
        }

        REGEX.captures(&ratio).and_then(|captures| {
            let from = captures.name("from").unwrap().as_str().parse::<u32>().ok();
            let to = captures.name("to").unwrap().as_str().parse::<u32>().ok();

            match (from, to) {
                (Some(from), Some(to)) => Some(StockSplitRatio::new(from, to)),
                _ => None,
            }
        }).ok_or_else(|| D::Error::custom(format!("Invalid stock split ratio: {:?}", ratio)))
    }
}

#[derive(Default)]
pub struct StockSplitController {
    symbols: HashMap<String, BTreeMap<DateTime, u32>>
}

impl StockSplitController {
    pub fn add(&mut self, time: DateOptTime, symbol: &str, divisor: u32) -> GenericResult<EmptyResult> {
        let split_time = time.or_min_time();
        let splits = self.symbols.entry(symbol.to_owned()).or_default();

        match splits.entry(split_time) {
            btree_map::Entry::Vacant(entry) => entry.insert(divisor),
            btree_map::Entry::Occupied(_) => return Err!(
                "Got a duplicated {} stock split for {}",
                symbol, format_date(time),
            ),
        };

        let mut full_divisor = dec!(1);

        for cur_divisor in splits.values().rev().copied() {
            full_divisor *= Decimal::from(cur_divisor);

            if dec!(1) / full_divisor * full_divisor != dec!(1) {
                splits.remove(&split_time).unwrap();
                return Ok(Err!("Got an unsupported stock split result divisor: {}", full_divisor));
            }
        }

        Ok(Ok(()))
    }

    pub fn get_multiplier(&self, symbol: &str, from_time: DateOptTime, to_time: DateOptTime) -> Decimal {
        let from_time = from_time.or_min_time();
        let to_time = to_time.or_min_time();
        let mut multiplier = dec!(1);

        let (start, end, divide) = if from_time < to_time {
            (from_time, to_time, false)
        } else if to_time < from_time {
            (to_time, from_time, true)
        } else {
            return multiplier;
        };

        let splits = match self.symbols.get(symbol) {
            Some(splits) => splits,
            None => return multiplier,
        };

        for (_, &divisor) in splits.range((Bound::Excluded(start), Bound::Included(end))) {
            multiplier *= Decimal::from(divisor);
        }

        if divide {
            multiplier = dec!(1) / multiplier;
            assert_eq!(dec!(1) * multiplier / multiplier, dec!(1));
        }

        multiplier
    }

    pub fn rename(&mut self, symbol: &str, new_symbol: &str) -> EmptyResult {
        if let Some((symbol, splits)) = self.symbols.remove_entry(symbol) {
            match self.symbols.entry(new_symbol.to_owned()) {
                hash_map::Entry::Vacant(entry) => {
                    entry.insert(splits);
                },
                hash_map::Entry::Occupied(_) => {
                    self.symbols.insert(symbol, splits);
                    return Err!("Stock split controller already has {} symbol", new_symbol);
                },
            };
        }
        Ok(())
    }
}

pub fn process_corporate_actions(statement: &mut BrokerStatement) -> EmptyResult {
    let corporate_actions = statement.corporate_actions.drain(..).collect::<Vec<_>>();

    for action in corporate_actions {
        process_corporate_action(statement, action)?;
    }

    Ok(())
}

fn process_corporate_action(statement: &mut BrokerStatement, action: CorporateAction) -> EmptyResult {
    match action.action {
        CorporateActionType::StockSplit {ratio, from_change, to_change} => {
            process_stock_split(
                statement, action.time, &action.symbol,
                ratio, from_change, to_change,
            ).map_err(|e| format!(
                "Failed to process {} stock split from {}: {}",
                action.symbol, format_date(action.time), e,
            ))?;
        },

        CorporateActionType::Spinoff {ref symbol, quantity, ..} => {
            statement.stock_buys.push(StockBuy::new_corporate_action(
                &symbol, quantity, PurchaseTotalCost::new(),
                action.time, action.execution_date(),
            ));
            statement.sort_and_validate_stock_buys()?;
        },

        CorporateActionType::Rename {ref new_symbol} => {
            statement.rename_symbol(&action.symbol, &new_symbol, Some(action.time)).map_err(|e| format!(
                "Failed to process {} -> {} rename corporate action: {}",
                action.symbol, new_symbol, e))?;
        },
    };

    statement.corporate_actions.push(action);
    Ok(())
}

fn process_stock_split(
    statement: &mut BrokerStatement,
    split_time: DateOptTime, symbol: &str, ratio: StockSplitRatio,
    from_change: Option<Decimal>, to_change: Option<Decimal>,
) -> EmptyResult {
    // We have two algorithms of handling stock split:
    // * The first one is most straightforward, but it can be applied only to simple splits
    //   that don't produce complex stock fractions.
    // * The second one is much complex (to write and to explain to tax inspector), loses
    //   long term investment tax exemptions, but can be applied to any stock split.

    if ratio.from == 1 {
        if !statement.stock_buys.iter().any(|trade| {
            trade.symbol == symbol && !trade.is_sold() && trade.conclusion_time < split_time
        }) {
            return Err!("The portfolio has no open {} position at {}", symbol, format_date(split_time));
        }

        match statement.stock_splits.add(split_time, symbol, ratio.to)? {
            Ok(()) => return Ok(()),
            Err(e) => debug!("{}: {}. Using complex stock split algorithm.", symbol, e),
        };
    };

    process_complex_stock_split(statement, split_time, symbol, ratio, from_change, to_change)
}

fn process_complex_stock_split(
    statement: &mut BrokerStatement,
    split_time: DateOptTime, symbol: &str, ratio: StockSplitRatio,
    from_change: Option<Decimal>, to_change: Option<Decimal>,
) -> EmptyResult {
    statement.process_trades(Some(split_time))?;

    let mut quantity = dec!(0);
    let mut sell_sources = Vec::new();

    for stock_buy in &mut statement.stock_buys {
        if stock_buy.symbol != symbol || stock_buy.is_sold() || stock_buy.conclusion_time >= split_time {
            continue;
        }

        let multiplier = statement.stock_splits.get_multiplier(
            symbol, stock_buy.conclusion_time, split_time);

        let sell_source = stock_buy.sell(stock_buy.get_unsold(), multiplier);
        quantity += sell_source.quantity * sell_source.multiplier;
        sell_sources.push(sell_source);
    }

    if sell_sources.is_empty() {
        return Err!("The portfolio has no open {} position at {}", symbol, format_date(split_time));
    }

    let new_quantity = calculate_stock_split(quantity, ratio, from_change, to_change)?;
    debug!("{} stock split from {}: {} -> {}.",
        symbol, format_date(split_time.date), quantity, new_quantity);

    // We create sell+buy trades with execution date equal to conclusion date and insert them to the
    // beginning of the list to be sure that they will be placed before any corporate action related
    // trades issued by broker after list sorting.

    let (sell, buy) = convert_stocks(symbol, quantity, new_quantity, split_time, sell_sources);

    statement.stock_sells.insert(0, sell);
    statement.sort_and_validate_stock_sells()?;

    statement.stock_buys.insert(0, buy);
    statement.sort_and_validate_stock_buys()?;

    Ok(())
}

fn calculate_stock_split(
    quantity: Decimal, ratio: StockSplitRatio,
    from_change: Option<Decimal>, to_change: Option<Decimal>,
) -> GenericResult<Decimal> {
    Ok(if let Some(lots) = get_stock_split_lots(quantity, ratio) {
        let new_quantity = (ratio.to.to_u64().unwrap() * lots.to_u64().unwrap()).into();

        if from_change.is_some() || to_change.is_some() {
            let from_change = from_change.unwrap_or_else(Decimal::zero);
            let to_change = to_change.unwrap_or_else(Decimal::zero);

            if quantity - from_change + to_change != new_quantity {
                return Err!(
                    "Got an unexpected stock split parameters: {} - {} + {} != {}",
                    quantity, from_change, to_change, new_quantity);
            }
        }

        new_quantity
    } else {
        let new_quantity = match (from_change, to_change) {
            (Some(from_change), Some(to_change)) if from_change == quantity => to_change,
            _ => return Err!(
                "Unsupported stock split: {} for {} when portfolio has {} shares",
                ratio.to, ratio.from, quantity),
        };

        let expected_quantity = quantity / Decimal::from(ratio.from) * Decimal::from(ratio.to);
        if util::round(expected_quantity, 2) != util::round(new_quantity, 2) {
            return Err!(
                "Unexpected result of stock split: {} / {} * {} = {} when {} is expected",
                quantity, ratio.from, ratio.to, new_quantity, expected_quantity);
        }

        new_quantity
    })
}

fn get_stock_split_lots(quantity: Decimal, ratio: StockSplitRatio) -> Option<u32> {
    if !quantity.fract().is_zero() {
        return None;
    }

    quantity.to_u32().and_then(|quantity| {
        if quantity % ratio.from == 0 {
            Some(quantity / ratio.from)
        } else {
            None
        }
    })
}

fn convert_stocks(
    symbol: &str, old_quantity: Decimal, new_quantity: Decimal,
    conclusion_time: DateOptTime, sell_sources: Vec<StockSellSource>,
) -> (StockSell, StockBuy) {
    let mut cost = PurchaseTotalCost::new();
    for source in &sell_sources {
        cost.add(&source.cost);
    }

    let mut sell = StockSell::new_corporate_action(
        symbol, old_quantity, conclusion_time, conclusion_time.date);
    sell.process(sell_sources);

    let buy = StockBuy::new_corporate_action(
        symbol, new_quantity, cost, conclusion_time, conclusion_time.date);

    (sell, buy)
}