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

    // FIXME(konishchev): Implement
    pub fn get_multiplier(&self, _symbol: &str, _start_date: Date, _end_date: Date) -> Decimal {
        dec!(1)
    }
}