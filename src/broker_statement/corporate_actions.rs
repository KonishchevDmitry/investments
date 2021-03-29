use std::collections::{HashMap, BTreeMap, btree_map};

use lazy_static::lazy_static;
use log::debug;
use num_traits::{ToPrimitive, Zero};
use regex::{self, Regex};
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::formatting::format_date;
use crate::trades::calculate_price;
use crate::types::{Date, Decimal};
use crate::util::{self, deserialize_date};

use super::BrokerStatement;
use super::trades::{StockBuy, StockBuyType, StockSell, StockSellType, StockSellSource};

#[derive(Deserialize, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CorporateAction {
    // Date when the corporate action has occurred (assuming that the changes have been made at the
    // end of trading day).
    #[serde(deserialize_with = "deserialize_date")]
    pub date: Date,

    // Report date from IB statements. Typically, it's T+1 date, so use it as trade execution date.
    #[serde(skip)]
    pub report_date: Option<Date>,

    pub symbol: String,
    #[serde(flatten)]
    pub action: CorporateActionType,
}

impl CorporateAction {
    fn execution_date(&self) -> Date {
        self.report_date.unwrap_or_else(|| self.date.succ())
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
        CorporateActionType::StockSplit {ratio, from_change, to_change} => {
            // FIXME(konishchev): Support
            // FIXME(konishchev): Tax exemptions
            if ratio.from == 1 {
                statement.stock_splits.add(action.date, &action.symbol, ratio.to)?;
            } else {
                process_complex_stock_split(
                    statement, action.date, &action.symbol, ratio,
                    from_change, to_change)?;
            }
        }
        CorporateActionType::Spinoff {ref symbol, quantity, ref currency} => {
            let zero = Cash::new(&currency, dec!(0));
            statement.stock_buys.push(StockBuy::new(
                StockBuyType::CorporateAction, &symbol, quantity, zero, zero, zero,
                action.date, action.execution_date(), false,
            ));
            statement.sort_and_validate_stock_buys()?;
        },
    };

    statement.corporate_actions.push(action);
    Ok(())
}

fn process_complex_stock_split(
    statement: &mut BrokerStatement,
    date: Date, symbol: &str, ratio: StockSplitRatio,
    from_change: Option<Decimal>, to_change: Option<Decimal>,
) -> EmptyResult {
    statement.process_trades(Some(date))?;

    let mut quantity = dec!(0);
    let mut sell_sources = Vec::new();

    // FIXME(konishchev): HERE
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
            "Got {} stock split at {} when portfolio has no open positions with it",
            symbol, format_date(date));
    }
    assert_ne!(quantity, dec!(0));

    let new_quantity = calculate_stock_split(date, symbol, quantity, ratio, from_change, to_change)?;
    debug!("{} stock split from {}: {} -> {}.", symbol, format_date(date), quantity, new_quantity);

    // FIXME(konishchev): Wrap error
    let (sell, buy) = convert_stocks(date, symbol, quantity, new_quantity, sell_sources)?;

    statement.stock_sells.push(sell);
    statement.sort_and_validate_stock_sells()?;

    statement.stock_buys.push(buy);
    statement.sort_and_validate_stock_buys()?;

    Ok(())
}

fn calculate_stock_split(
    date: Date, symbol: &str, quantity: Decimal, ratio: StockSplitRatio,
    from_change: Option<Decimal>, to_change: Option<Decimal>,
) -> GenericResult<Decimal> {
    Ok(if let Some(lots) = get_stock_split_lots(quantity, ratio) {
        let new_quantity = (ratio.to.to_u64().unwrap() * lots.to_u64().unwrap()).into();

        if from_change.is_some() || to_change.is_some() {
            let from_change = from_change.unwrap_or_else(Decimal::zero);
            let to_change = to_change.unwrap_or_else(Decimal::zero);

            if quantity - from_change + to_change != new_quantity {
                return Err!(
                    "Got unexpected parameters for {} stock split from {}: {} - {} + {} != {}",
                    symbol, format_date(date), quantity, from_change, to_change, new_quantity);
            }
        }

        new_quantity
    } else {
        let new_quantity = match (from_change, to_change) {
            (Some(from_change), Some(to_change)) if from_change == quantity => to_change,
            _ => return Err!(
                "Unsupported {} stock split from {}: {} for {} when portfolio has {} shares",
                symbol, format_date(date), ratio.to, ratio.from, quantity),
        };

        let expected_quantity = quantity / Decimal::from(ratio.from) * Decimal::from(ratio.to);
        if util::round(expected_quantity, 2) != util::round(new_quantity, 2) {
            return Err!(
                "Unexpected result of {} stock split from {}: {} / {} * {} = {} when {} is expected",
                symbol, format_date(date), quantity, ratio.from, ratio.to, new_quantity, expected_quantity);
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
    date: Date, symbol: &str, old_quantity: Decimal, new_quantity: Decimal,
    sell_sources: Vec<StockSellSource>,
) -> GenericResult<(StockSell, StockBuy)> {
    let mut volume = Cash::new(sell_sources.first().unwrap().volume.currency, dec!(0));
    let mut commission = Cash::new(sell_sources.first().unwrap().commission.currency, dec!(0));

    for source in &sell_sources {
        volume.add_assign(source.volume).and_then(|_| {
            commission.add_assign(source.commission)
        }).map_err(|_| "Buy trades have mixed currency")?;
    }

    if commission.currency == volume.currency {
        volume.amount += commission.amount;
        commission.amount = dec!(0);
    }

    // FIXME(konishchev): Local cost
    let sell_price = calculate_price(old_quantity, volume)?;
    let mut sell = StockSell::new(
        // FIXME(konishchev): Sell type
        StockSellType::CorporateAction, symbol, old_quantity, sell_price, volume, commission,
        date, date, false, false,
    );
    sell.process(sell_sources);

    // FIXME(konishchev): Local cost
    let buy_price = calculate_price(new_quantity, volume)?;
    let buy = StockBuy::new(
        // FIXME(konishchev): Buy type
        StockBuyType::CorporateAction, symbol, new_quantity,
        buy_price, volume, commission, date, date, false,
    );

    Ok((sell, buy))
}