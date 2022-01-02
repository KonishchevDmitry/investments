use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::default::Default;
use std::fmt::{self, Display};

use cusip::CUSIP;
use itertools::Itertools;
use isin::ISIN;
use serde::Deserialize;
use serde::de::Deserializer;

use crate::core::{GenericResult, EmptyResult};
use crate::exchanges::Exchanges;
use crate::localities::Jurisdiction;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum InstrumentId {
    Symbol(String),
    Isin(ISIN),
    Name(String),
    InternalId(String), // Some broker-specific ID
}

impl Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id: &dyn Display = match self {
            InstrumentId::Symbol(symbol) => symbol,
            InstrumentId::Isin(isin) => isin,
            InstrumentId::Name(name) => name,
            InstrumentId::InternalId(id) => id,
        };
        id.fmt(f)
    }
}

#[derive(Default, Clone)]
pub struct InstrumentInternalIds(HashMap<String, String>);

impl InstrumentInternalIds {
    fn get_symbol(&self, id: &str) -> GenericResult<&str> {
        Ok(self.0.get(id).ok_or_else(|| format!(concat!(
            "Unable to determine stock symbol by its broker-specific internal ID ({}). ",
            "Please specify the mapping via `instrument_internal_ids` configuration option"
        ), id))?)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<InstrumentInternalIds, D::Error>
        where D: Deserializer<'de>
    {
        Ok(InstrumentInternalIds(Deserialize::deserialize(deserializer)?))
    }
}

pub struct InstrumentInfo {
    instruments: HashMap<String, Instrument>,
    internal_ids: Option<InstrumentInternalIds>,
}

impl InstrumentInfo {
    pub fn new() -> InstrumentInfo {
        InstrumentInfo {
            instruments: HashMap::new(),
            internal_ids: None,
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.instruments.is_empty()
    }

    pub fn set_internal_ids(&mut self, ids: InstrumentInternalIds) {
        self.internal_ids.replace(ids);
    }

    pub fn get_name(&self, symbol: &str) -> String {
        if let Some(name) = self.instruments.get(symbol).and_then(|info| info.name.as_ref()) {
            format!("{} ({})", name, symbol)
        } else {
            symbol.to_owned()
        }
    }

    pub fn add(&mut self, symbol: &str) -> GenericResult<&mut Instrument> {
        match self.instruments.entry(symbol.to_owned()) {
            Entry::Vacant(entry) => Ok(entry.insert(Instrument::new(symbol))),
            Entry::Occupied(_) => Err!("Duplicated security symbol: {}", symbol),
        }
    }

    pub fn get(&self, symbol: &str) -> Option<&Instrument> {
        self.instruments.get(symbol)
    }

    pub fn get_or_add(&mut self, symbol: &str) -> &mut Instrument {
        match self.instruments.entry(symbol.to_owned()) {
            Entry::Vacant(entry) => entry.insert(Instrument::new(symbol)),
            Entry::Occupied(entry) => entry.into_mut(),
        }
    }

    pub fn get_or_add_by_id(&mut self, instrument_id: &InstrumentId) -> GenericResult<&Instrument> {
        Ok(match instrument_id {
            InstrumentId::Symbol(symbol) => {
                self.get_or_add(symbol)
            },

            InstrumentId::Isin(isin) => {
                let mut results = Vec::with_capacity(1);

                for instrument in self.instruments.values() {
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

                for instrument in self.instruments.values() {
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

            InstrumentId::InternalId(id) => {
                let symbol = self.internal_ids.as_ref().unwrap().get_symbol(id)?.to_owned();
                self.get_or_add(&symbol)
            },
        })
    }

    pub fn remap(&mut self, old_symbol: &str, new_symbol: &str) -> EmptyResult {
        if self.instruments.contains_key(new_symbol) {
            return Err!("The portfolio already has {} symbol", new_symbol);
        }

        if let Some(mut info) = self.instruments.remove(old_symbol) {
            info.symbol = new_symbol.to_owned();
            self.instruments.insert(new_symbol.to_owned(), info);
        }

        Ok(())
    }

    pub fn merge(&mut self, other: InstrumentInfo) {
        assert!(other.internal_ids.is_none());

        for (symbol, info) in other.instruments {
            match self.instruments.entry(symbol) {
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
    pub isin: HashSet<ISIN>,
    cusip: HashSet<CUSIP>,
    pub exchanges: Exchanges,
}

impl Instrument {
    fn new(symbol: &str) -> Instrument {
        Instrument {
            symbol:    symbol.to_owned(),
            name:      None,
            isin:      HashSet::new(),
            cusip:     HashSet::new(),
            exchanges: Exchanges::new_empty(),
        }
    }

    pub fn set_name(&mut self, name: &str) {
        self.name.replace(name.to_owned());
    }

    pub fn add_isin(&mut self, isin: ISIN) {
        self.isin.insert(isin);
    }

    pub fn add_cusip(&mut self, cusip: CUSIP) {
        self.cusip.insert(cusip);
    }

    pub fn get_taxation_type(&self, broker_jurisdiction: Jurisdiction) -> GenericResult<IssuerTaxationType> {
        let get_taxation_type = |issuer_jurisdiction: &str| -> IssuerTaxationType {
            if broker_jurisdiction == Jurisdiction::Russia && issuer_jurisdiction == Jurisdiction::Russia.code() {
                return IssuerTaxationType::TaxAgent;
            }
            IssuerTaxationType::Manual(Some(issuer_jurisdiction.to_owned()))
        };

        let mut result_taxation_type = if self.cusip.is_empty() {
            None
        } else {
            Some(get_taxation_type(Jurisdiction::Usa.code()))
        };

        for isin in &self.isin {
            let taxation_type = get_taxation_type(isin.prefix());

            if let Some(prev) = result_taxation_type.as_ref() {
                if *prev != taxation_type {
                    let ids = self.isin.iter().map(ToString::to_string)
                        .chain(self.cusip.iter().map(|id| format!("CUSIP:{}", id)))
                        .join(", ");

                    return Err!(
                        "Unable to determine {} taxation type: it has several ISIN with different jurisdictions: {}",
                        self.symbol, ids)
                }
            } else {
                result_taxation_type.replace(taxation_type);
            }
        }

        Ok(if let Some(taxation_type) = result_taxation_type {
            taxation_type
        } else if broker_jurisdiction == Jurisdiction::Russia {
            return Err!(
                "Unable to determine {} taxation type: there is no ISIN information for it in the broker statement",
                self.symbol);
        } else {
            IssuerTaxationType::Manual(None)
        })
    }

    fn merge(&mut self, other: Instrument) {
        if let Some(name) = other.name {
            self.name.replace(name);
        }
        self.isin.extend(other.isin);
        self.cusip.extend(other.cusip);
        self.exchanges.merge(other.exchanges);
    }
}

#[derive(Clone, PartialEq)]
pub enum IssuerTaxationType {
    Manual(Option<String>),

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

pub fn parse_isin(value: &str) -> GenericResult<ISIN> {
    Ok(value.parse().map_err(|_| format!("Invalid ISIN: {}", value))?)
}