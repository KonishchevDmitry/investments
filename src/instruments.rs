use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::default::Default;

use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;
use serde::de::Deserializer;

use crate::core::{GenericResult, EmptyResult};

pub struct InstrumentInternalIds(HashMap<String, String>);

impl Default for InstrumentInternalIds {
    fn default() -> InstrumentInternalIds {
        InstrumentInternalIds(HashMap::new())
    }
}

impl InstrumentInternalIds {
    pub fn get_symbol(&self, id: &str) -> GenericResult<&str> {
        Ok(self.0.get(id).ok_or_else(|| format!(concat!(
            "Unable to determine stock symbol by its internal ID ({}). ",
            "Please specify the mapping via `instrument_internal_ids` configuration option"
        ), id))?)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<InstrumentInternalIds, D::Error>
        where D: Deserializer<'de>
    {
        Ok(InstrumentInternalIds(Deserialize::deserialize(deserializer)?))
    }
}

pub struct InstrumentInfo(HashMap<String, Instrument>);

impl InstrumentInfo {
    pub fn new() -> InstrumentInfo {
        InstrumentInfo(HashMap::new())
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get_name(&self, symbol: &str) -> String {
        if let Some(name) = self.0.get(symbol).and_then(|info| info.name.as_ref()) {
            format!("{} ({})", name, symbol)
        } else {
            symbol.to_owned()
        }
    }

    pub fn add(&mut self, symbol: &str) -> GenericResult<&mut Instrument> {
        match self.0.entry(symbol.to_owned()) {
            Entry::Vacant(entry) => Ok(entry.insert(Instrument {
                name: None,
                isin: HashSet::new(),
            })),
            Entry::Occupied(_) => Err!("Duplicated security symbol: {}", symbol),
        }
    }

    pub fn get_or_add(&mut self, symbol: &str) -> &mut Instrument {
        match self.0.entry(symbol.to_owned()) {
            Entry::Vacant(entry) => entry.insert(Instrument {
                name: None,
                isin: HashSet::new(),
            }),
            Entry::Occupied(entry) => entry.into_mut(),
        }
    }

    pub fn remap(&mut self, old_symbol: &str, new_symbol: &str) -> EmptyResult {
        if self.0.contains_key(new_symbol) {
            return Err!("The portfolio already has {} symbol", new_symbol);
        }

        if let Some(info) = self.0.remove(old_symbol) {
            self.0.insert(new_symbol.to_owned(), info);
        }

        Ok(())
    }

    pub fn name_mapping(&self) -> HashMap<String, HashSet<String>> {
        let mut mapping = HashMap::<String, HashSet<String>>::new();

        for (symbol, info) in self.0.iter() {
            if let Some(ref name) = info.name {
                mapping.entry(name.to_owned()).or_default().insert(symbol.to_owned());
            }
        }

        mapping
    }

    pub fn merge(&mut self, other: InstrumentInfo) {
        for (symbol, info) in other.0 {
            match self.0.entry(symbol) {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().merge(info);
                },
                Entry::Vacant(entry) => {
                    entry.insert(info);
                },
            };
        }
    }

    pub fn merge_symbols(&mut self, master_symbol: &str, slave_symbol: &str, strict: bool) -> EmptyResult {
        let mut result = match self.0.remove(slave_symbol) {
            Some(info) => info,
            None => if strict {
                return Err!("The broker statement has no any activity for {:?} symbol", slave_symbol);
            } else {
                return Ok(());
            },
        };

        if let Some(info) = self.0.remove(master_symbol) {
            result.merge(info);
        }

        self.0.insert(master_symbol.to_owned(), result);
        Ok(())
    }
}

pub struct Instrument {
    name: Option<String>,
    isin: HashSet<String>,
}

impl Instrument {
    pub fn set_name(&mut self, name: &str) {
        self.name.replace(name.to_owned());
    }

    pub fn add_isin(&mut self, isin: &str) -> EmptyResult {
        lazy_static! {
            static ref REGEX: Regex = Regex::new(r"^[A-Z]{2}[A-Z0-9]{9}[0-9]$").unwrap();
        }

        if !REGEX.is_match(isin) {
            return Err!("Invalid ISIN: {:?}", isin);
        }

        self.isin.insert(isin.to_owned());
        Ok(())
    }

    fn merge(&mut self, other: Instrument) {
        if let Some(name) = other.name {
            self.name.replace(name);
        }
        self.isin.extend(other.isin);
    }
}