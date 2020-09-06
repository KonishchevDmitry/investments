use std::collections::{HashMap, BTreeMap, btree_map};

use crate::core::EmptyResult;
use crate::formatting::format_date;
use crate::types::{Date, Decimal};

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CorporateAction {
    pub date: Date,
    pub symbol: String,
    pub action: CorporateActionType,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum CorporateActionType {
    StockSplit(u32),
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

        Ok(())
    }

    // FIXME(konishchev): Implement
    pub fn get_quantity(&self, _date: Date, _symbol: &str, quantity: Decimal) -> Decimal {
        quantity
    }
}