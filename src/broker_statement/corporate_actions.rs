use std::collections::{HashMap, BTreeMap, btree_map};

use lazy_static::lazy_static;
use log::debug;
use num_traits::{ToPrimitive, Zero};
use regex::{self, Regex};
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::formatting::format_date;
use crate::types::{Date, Decimal};
use crate::util::deserialize_date;

use super::BrokerStatement;
use super::trades::{StockBuy, StockSellSource, StockSource};

#[derive(Deserialize, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CorporateAction {
    #[serde(deserialize_with = "deserialize_date")]
    pub date: Date,
    pub symbol: String,
    #[serde(flatten)]
    pub action: CorporateActionType,
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

    // See https://github.com/KonishchevDmitry/investments/issues/20 for details
    #[serde(skip)]
    Spinoff {
        date: Date,
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

#[derive(Default, Debug)]
pub struct StockSplitController {
    symbols: HashMap<String, BTreeMap<Date, u32>>
}

impl StockSplitController {
    pub fn add(&mut self, date: Date, symbol: &str, divisor: u32) -> EmptyResult {
        let splits = self.symbols.entry(symbol.to_owned()).or_default();

        match splits.entry(date) {
            btree_map::Entry::Vacant(entry) => entry.insert(divisor),
            btree_map::Entry::Occupied(_) => return Err!(
                "Got a duplicated {} stock split for {}",
                 symbol, format_date(date),
            ),
        };

        let mut full_divisor = dec!(1);

        for cur_divisor in splits.values().rev().copied() {
            full_divisor *= Decimal::from(cur_divisor);

            if dec!(1) / full_divisor * full_divisor != dec!(1) {
                splits.remove(&date).unwrap();
                return Err!("Got an unsupported stock split result divisor: {}", full_divisor);
            }
        }

        Ok(())
    }

    pub fn get_multiplier(&self, symbol: &str, from_date: Date, to_date: Date) -> Decimal {
        let mut multiplier = dec!(1);

        let (start, end, divide) = if from_date < to_date {
            (from_date.succ(), to_date, false)
        } else if to_date < from_date {
            (to_date.succ(), from_date, true)
        } else {
            return multiplier;
        };

        let splits = match self.symbols.get(symbol) {
            Some(splits) => splits,
            None => return multiplier,
        };

        for (_, &divisor) in splits.range(start..=end) {
            multiplier *= Decimal::from(divisor);
        }

        if divide {
            multiplier = dec!(1) / multiplier;
            assert_eq!(dec!(1) * multiplier / multiplier, dec!(1));
        }

        multiplier
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
        // FIXME(konishchev): Support other fields
        CorporateActionType::StockSplit {ratio, ..} => {
            // FIXME(konishchev): Support
            // FIXME(konishchev): Tax exemptions
            if false {
                process_complex_stock_split(statement, action.date, &action.symbol, ratio)?;
            } else {
                assert_eq!(ratio.from, 1);
                statement.stock_splits.add(action.date, &action.symbol, ratio.to)?;
            }
        }
        CorporateActionType::Spinoff {date, ref symbol, quantity, ref currency} => {
            let zero = Cash::new(&currency, dec!(0));
            statement.stock_buys.push(StockBuy::new(
                &symbol, quantity, StockSource::CorporateAction, zero, zero, zero,
                date, action.date, false,
            ));
        },
    };

    statement.corporate_actions.push(action);
    Ok(())
}

fn process_complex_stock_split(
    statement: &mut BrokerStatement,
    date: Date, symbol: &str, ratio: StockSplitRatio,
) -> EmptyResult {
    statement.process_trades(Some(date))?;

    let mut quantity = dec!(0);
    let mut sell_sources = Vec::new();

    for stock_buy in &mut statement.stock_buys {
        if stock_buy.symbol != symbol || stock_buy.is_sold() || stock_buy.conclusion_date >= date {
            continue;
        }

        let multiplier = statement.stock_splits.get_multiplier(
            symbol, stock_buy.conclusion_date, date);

        let sell_source = stock_buy.sell(stock_buy.get_unsold(), multiplier);
        quantity += sell_source.quantity * sell_source.multiplier;
        sell_sources.push(sell_source);
    }

    if sell_sources.is_empty() {
        return Err!(
            "Got {} stock split ({}) when portfolio has no open positions with it",
            symbol, format_date(date));
    }

    let lots = get_stock_split_lots(quantity, ratio).ok_or_else(|| format!(
        "Unsupported {} stock split from {}: {} for {} when portfolio has {} shares",
        symbol, format_date(date), ratio.to, ratio.from, quantity,
    ))?;

    let new_quantity: Decimal = (ratio.to.to_u64().unwrap() * lots.to_u64().unwrap()).into();
    debug!("{} stock split: {} -> {}.", symbol, quantity, new_quantity);

    convert_stocks(symbol, quantity, sell_sources)?;
    // FIXME(konishchev): HERE
    /*
    if false {
        self.stock_sells.push(StockSell::new(
            &action.symbol, from_change.unwrap(),
            Cash::new("USD", dec!(0)), Cash::new("USD", dec!(0)), Cash::new("USD", dec!(0)),
            action.date, action.date, false, false,
        ));
    }
    self.stock_buys.push(StockBuy::new(
        &action.symbol, to_change.unwrap(), StockSource::CorporateAction,
        Cash::new("USD", dec!(0)), Cash::new("USD", dec!(0)), Cash::new("USD", dec!(0)),
        action.date, action.date, false,
    ));
    */

    Ok(())
}

fn convert_stocks(
    symbol: &str, _old_quantity: Decimal, sell_sources: Vec<StockSellSource>,
) -> EmptyResult {
    let mut volume = Cash::new(sell_sources.first().unwrap().volume.currency, dec!(0));
    let mut commission = Cash::new(sell_sources.first().unwrap().commission.currency, dec!(0));

    for source in &sell_sources {
        volume.add_assign(source.volume).and_then(|_| {
            commission.add_assign(source.commission)
        }).map_err(|_| format!("Unsupported {} stock split: buy trades have mixed currency", symbol))?;
    }

    // StockSell::new(symbol, old_quantity)

    Ok(())
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