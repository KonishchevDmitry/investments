use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::default::Default;
use std::fmt;

use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;
use serde::de::Deserializer;

use crate::core::{GenericResult, EmptyResult};
use crate::localities::Jurisdiction;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum InstrumentId {
    Symbol(String),
    Isin(String),
    Name(String),
}

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", match self {
            InstrumentId::Symbol(symbol) => symbol,
            InstrumentId::Isin(isin) => isin,
            InstrumentId::Name(name) => name,
        })
    }
}

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
                symbol: symbol.to_owned(),
                name:   None,
                isin:   HashSet::new(),
            })),
            Entry::Occupied(_) => Err!("Duplicated security symbol: {}", symbol),
        }
    }

    pub fn get_or_add(&mut self, symbol: &str) -> &mut Instrument {
        match self.0.entry(symbol.to_owned()) {
            Entry::Vacant(entry) => entry.insert(Instrument {
                symbol: symbol.to_owned(),
                name:   None,
                isin:   HashSet::new(),
            }),
            Entry::Occupied(entry) => entry.into_mut(),
        }
    }

    pub fn get_or_add_by_id(&mut self, instrument_id: &InstrumentId) -> GenericResult<&Instrument> {
        Ok(match instrument_id {
            InstrumentId::Symbol(symbol) => self.get_or_add(symbol),
            InstrumentId::Isin(isin) => {
                let mut results = Vec::with_capacity(1);

                for instrument in self.0.values() {
                    if instrument.isin.contains(isin) {
                        results.push(instrument);
                    }
                }

                match results.len() {
                    1 => results.first().unwrap(),
                    0 => return Err!("Unable to find instrument information by its ISIN: {}", isin),
                    _ => return Err!(
                        "Unable to map {} ISIN to instrument symbol: it maps into several symbols: {}",
                        isin, results.iter().map(|result| &result.symbol).join(", ")),
                }
            },
            InstrumentId::Name(name) => {
                let mut results = Vec::with_capacity(1);

                for instrument in self.0.values() {
                    if matches!(instrument.name, Some(ref other) if other == name) {
                        results.push(instrument);
                    }
                }

                match results.len() {
                    1 => results.first().unwrap(),
                    0 => return Err!("Unable to find instrument information by its name: {:?}", name),
                    _ => return Err!(
                        "Unable to map {:?} to instrument symbol: it maps into several symbols: {}",
                        name, results.iter().map(|result| &result.symbol).join(", ")),
                }
            },
        })
    }

    pub fn remap(&mut self, old_symbol: &str, new_symbol: &str) -> EmptyResult {
        if self.0.contains_key(new_symbol) {
            return Err!("The portfolio already has {} symbol", new_symbol);
        }

        if let Some(mut info) = self.0.remove(old_symbol) {
            info.symbol = new_symbol.to_owned();
            self.0.insert(new_symbol.to_owned(), info);
        }

        Ok(())
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
}

pub struct Instrument {
    pub symbol: String,
    name: Option<String>,
    isin: HashSet<String>,
}

impl Instrument {
    pub fn set_name(&mut self, name: &str) {
        self.name.replace(name.to_owned());
    }

    pub fn add_isin(&mut self, isin: &str) -> EmptyResult {
        parse_isin(isin)?;
        self.isin.insert(isin.to_owned());
        Ok(())
    }

    pub fn get_taxation_type(&self, broker_jurisdiction: Jurisdiction) -> GenericResult<IssuerTaxationType> {
        let country_of_residence = Jurisdiction::Russia.code();

        let get_taxation_type = match broker_jurisdiction {
            Jurisdiction::Russia => |isin| -> GenericResult<IssuerTaxationType> {
                let issuer_jurisdiction = parse_isin(isin)?;

                Ok(if issuer_jurisdiction == country_of_residence {
                    IssuerTaxationType::TaxAgent
                } else {
                    IssuerTaxationType::Manual(issuer_jurisdiction.to_owned())
                })
            },
            Jurisdiction::Usa => {
                // See https://github.com/KonishchevDmitry/investments/blob/master/docs/taxes.md#foreign-income-jurisdiction
                // for details.
                let income_jurisdiction = broker_jurisdiction.code();
                return Ok(IssuerTaxationType::Manual(income_jurisdiction.to_owned()));
            }
        };

        let mut result_taxation_type = None;

        for isin in &self.isin {
            let taxation_type = get_taxation_type(isin)?;

            if let Some(prev) = result_taxation_type.as_ref() {
                if *prev != taxation_type {
                    return Err!(
                        "Unable to determine {} taxation type: it has several ISIN with different jurisdictions: {}",
                        self.symbol, self.isin.iter().join(", "))
                }
            } else {
                result_taxation_type.replace(taxation_type);
            }
        }

        Ok(result_taxation_type.ok_or_else(|| format!(
            "Unable to determine {} taxation type: there is no ISIN information for it",
            self.symbol))?)
    }

    fn merge(&mut self, other: Instrument) {
        if let Some(name) = other.name {
            self.name.replace(name);
        }
        self.isin.extend(other.isin);
    }
}

#[derive(Clone, PartialEq)]
pub enum IssuerTaxationType {
    Manual(String),

    // Russian brokers withhold tax for dividends issued by companies with Russian jurisdiction and
    // don't withhold for other jurisdictions or Russian companies traded through ADR/GDR.
    //
    // Withheld tax may be less than 13%. It may be even zero if company distributes dividends from
    // other companies for which tax has been already withheld.
    //
    // See https://smart-lab.ru/company/tinkoff_invest/blog/631922.php for details.
    TaxAgent,
}

pub const ISIN_REGEX: &str = r"[A-Z]{2}[A-Z0-9]{9}[0-9]";

fn parse_isin(isin: &str) -> GenericResult<&str> {
    lazy_static! {
        static ref REGEX: Regex = Regex::new(&format!("^{}$", ISIN_REGEX)).unwrap();
    }

    if !REGEX.is_match(isin) {
        return Err!("Invalid ISIN: {:?}", isin);
    }

    Ok(&isin[..2])
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(isin, country,
        case("US7802592060", "US"),
        case("RU0009084396", "RU"),
    )]
    fn isin_parsing(isin: &str, country: &str) {
        assert_eq!(parse_isin(isin).unwrap(), country);
    }
}