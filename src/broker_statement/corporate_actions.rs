use std::collections::{HashMap, BTreeMap, btree_map};

use lazy_static::lazy_static;
use regex::{self, Regex};
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::EmptyResult;
use crate::formatting::format_date;
use crate::types::{Date, Decimal};
use crate::util::deserialize_date;

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
        #[serde(skip)]
        from: u32,

        #[serde(rename = "ratio", deserialize_with = "deserialize_ratio")]
        to: u32,
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

fn deserialize_ratio<'de, D>(deserializer: D) -> Result<u32, D::Error>
    where D: Deserializer<'de>
{
    let ratio: String = Deserialize::deserialize(deserializer)?;

    lazy_static! {
        static ref REGEX: Regex = Regex::new(r"^(?P<divisor>\d+):(?P<dividend>\d+)$").unwrap();
    }

    let (divisor, dividend) = REGEX.captures(&ratio).and_then(|captures| {
        let divisor = captures.name("divisor").unwrap().as_str().parse::<u32>().ok();
        let dividend = captures.name("dividend").unwrap().as_str().parse::<u32>().ok();

        match (divisor, dividend) {
            (Some(divisor), Some(dividend)) if divisor > 0 && dividend > 0 => Some((divisor, dividend)),
            _ => None,
        }
    }).ok_or_else(|| D::Error::custom(format!("Invalid stock split ratio: {:?}", ratio)))?;

    if dividend != 1 {
        return Err(D::Error::custom(format!("Unsupported stock split ratio: {:?}", ratio)));
    }

    Ok(divisor)
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