use std::cmp::Ordering;
use std::collections::{HashMap, BTreeMap, hash_map, btree_map};
use std::ops::Bound;

use lazy_static::lazy_static;
use log::debug;
use num_traits::ToPrimitive;
use regex::{self, Regex};
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::formatting::format_date;
use crate::localities::Jurisdiction;
use crate::time::{Date, DateTime, DateOptTime, deserialize_date_opt_time};
use crate::types::Decimal;

use super::BrokerStatement;
use super::trades::{StockBuy, StockSell, StockSellSource, PurchaseTotalCost};

#[derive(Deserialize, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
// Don't use #[serde(deny_unknown_fields)] because of #[serde(flatten)]
pub struct CorporateAction {
    // Date + time when the corporate action has occurred. If time is not present, assuming:
    // * The changes are made at the end of the trading day.
    // * All trade operations from this day are assumed to be issued after the corporate action has
    //   occurred and are actually a part of the corporate action.
    #[serde(rename="date", deserialize_with = "deserialize_date_opt_time")]
    pub time: DateOptTime,

    // Report date from broker statements. Typically, it's T+1 date, so use it as trade execution date.
    #[serde(skip)]
    pub report_date: Option<Date>,

    pub symbol: String,
    #[serde(flatten)]
    pub action: CorporateActionType,
}

impl CorporateAction {
    fn execution_date(&self) -> Date {
        self.report_date.unwrap_or_else(|| self.time.date.succ_opt().unwrap())
    }
}

#[derive(Deserialize, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "type", rename_all="kebab-case")]
pub enum CorporateActionType {
    // See https://github.com/KonishchevDmitry/investments/issues/73 for details
    //
    // Intended for events similar to delisting of FinEx FXRB fund which lost all its assets and has been closed
    // (https://finex-etf.ru/university/news/chto_delisting_fxrb_znachit_dlya_investorov/)
    //
    // Please note that we can't balance losses and profits between securities traded on organized securities market and
    // securities that aren't traded on organized securities market (see Article 220.1 of the Tax Code of the Russian
    // Federation or https://www.nalog.gov.ru/rn77/taxation/taxes/ndfl/nalog_vichet/nv_ubit/ details), so for now we
    // just ignore such events in all tax calculation.
    Delisting {
        quantity: Decimal,
    },

    #[serde(skip)]
    Liquidation {
        quantity: Decimal,
        price: Decimal,
        volume: Decimal,
        currency: String,
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
    },

    // There are two types of stock dividend (see https://github.com/KonishchevDmitry/investments/issues/27#issuecomment-802212517)
    // At this time we support only one of them.
    StockDividend {
        stock: Option<String>,
        quantity: Decimal,
    },

    // Depending on the source we might have the following split info:
    // 1. Only ratio is provided: use this value with no ability to check the result.
    // 2. In addition to ratio, withdrawal and/or deposit amount is provided: use the ratio value and validate the
    //    result against the processed deposits/withdrawals.
    StockSplit {
        ratio: StockSplitRatio,

        #[serde(skip)]
        withdrawal: Option<Decimal>,

        #[serde(skip)]
        deposit: Option<Decimal>,
    },

    // Allows existing shareholders to purchase shares of a secondary offering, usually at a
    // discounted price. Doesn't affects anything, so can be ignored.
    #[serde(skip)]
    SubscribableRightsIssue,
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

        let (start, end, divide) = match from_time.cmp(&to_time) {
            Ordering::Less => (from_time, to_time, false),
            Ordering::Greater => (to_time, from_time, true),
            Ordering::Equal => return multiplier,
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
        CorporateActionType::Delisting {quantity} => {
            statement.stock_sells.push(StockSell::new_corporate_action(
                &action.symbol, quantity, action.time, action.execution_date()));
            statement.sort_and_validate_stock_sells()?;
        },

        CorporateActionType::Liquidation {quantity, price, volume, ref currency} => {
            let price = Cash::new(currency, price);
            let volume = Cash::new(currency, volume);
            let commission = Cash::zero(currency);

            statement.stock_sells.push(StockSell::new_trade(
                &action.symbol, quantity, price, volume, commission,
                action.time, action.execution_date(), false));
            statement.sort_and_validate_stock_sells()?;
        },

        CorporateActionType::Rename {ref new_symbol} => {
            statement.rename_symbol(&action.symbol, new_symbol, Some(action.time), true).map_err(|e| format!(
                "Failed to process {} -> {} rename corporate action: {}",
                action.symbol, new_symbol, e))?;
        },

        CorporateActionType::Spinoff {ref symbol, quantity, ..} => {
            statement.stock_buys.push(StockBuy::new_corporate_action(
                symbol, quantity, PurchaseTotalCost::new(),
                action.time, action.execution_date(),
            ));
            statement.sort_and_validate_stock_buys()?;
        },

        CorporateActionType::StockDividend {ref stock, quantity} => {
            let symbol = stock.as_ref().unwrap_or(&action.symbol);
            statement.stock_buys.push(StockBuy::new_corporate_action(
                symbol, quantity, PurchaseTotalCost::new(),
                action.time, action.execution_date(),
            ));
            statement.sort_and_validate_stock_buys()?;
        },

        CorporateActionType::StockSplit {ratio, withdrawal, deposit} => {
            process_stock_split(
                statement, action.time, &action.symbol,
                ratio, withdrawal, deposit,
            ).map_err(|e| format!(
                "Failed to process {} stock split from {}: {}",
                action.symbol, format_date(action.time), e,
            ))?;
        },

        CorporateActionType::SubscribableRightsIssue => {},
    };

    statement.corporate_actions.push(action);
    Ok(())
}

fn process_stock_split(
    statement: &mut BrokerStatement,
    split_time: DateOptTime, symbol: &str, ratio: StockSplitRatio,
    withdrawal: Option<Decimal>, deposit: Option<Decimal>,
) -> EmptyResult {
    // We have two algorithms of handling stock split:
    // * The first one is most straightforward, but it can be applied only to simple splits
    //   that don't produce complex stock fractions.
    // * The second one is much complex (to write and to explain to tax inspector), loses
    //   long term investment tax exemptions, but can be applied to any stock split.
    //
    // We always use the second one for all Russian brokers, because they use it instead of the
    // first one and reset FIFO and LTO by this fact. See docs/brokers.md#stock-splits-in-russian-brokers
    // for details.

    if ratio.from == 1 && statement.broker.type_.jurisdiction() != Jurisdiction::Russia {
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

    process_complex_stock_split(statement, split_time, symbol, ratio, withdrawal, deposit)
}

fn process_complex_stock_split(
    statement: &mut BrokerStatement,
    split_time: DateOptTime, symbol: &str, ratio: StockSplitRatio,
    withdrawal: Option<Decimal>, deposit: Option<Decimal>,
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

    let new_quantity = calculate_stock_split(quantity, ratio, withdrawal, deposit)?;
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
    quantity: Decimal, ratio: StockSplitRatio, withdrawal: Option<Decimal>, deposit: Option<Decimal>,
) -> GenericResult<Decimal> {
    // We know the result quantity, so are able to check the inputs
    if withdrawal.is_some() || deposit.is_some() {
        let new_quantity = quantity - withdrawal.unwrap_or_default() + deposit.unwrap_or_default();
        let expected_quantity = quantity / Decimal::from(ratio.from) * Decimal::from(ratio.to);

        // Brokers round the result to some finite value
        let error = (new_quantity - expected_quantity).abs();
        if new_quantity.is_zero() || expected_quantity.is_zero() ||
            error / std::cmp::min(new_quantity, expected_quantity) > dec!(0.00001) {
            return Err!(
                "Unexpected result of stock split: {} / {} * {} = {} when {} is expected",
                quantity, ratio.from, ratio.to, new_quantity, expected_quantity);
        }

        return Ok(new_quantity)
    }

    if ratio.from == 1 || quantity.fract().is_zero() && quantity.to_u32().map(|quantity| {
        quantity % ratio.from == 0
    }).unwrap_or_default() {
        return Ok(quantity / Decimal::from(ratio.from) * Decimal::from(ratio.to));
    }

    Err!("Unsupported stock split: {} for {} when portfolio has {} shares",
         ratio.to, ratio.from, quantity)
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