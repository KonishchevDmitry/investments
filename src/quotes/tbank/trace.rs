use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
use log::{Level, log_enabled, trace};

use crate::core::EmptyResult;

use super::api::RealExchange;

pub struct InstrumentTrace {
    name: &'static str,
    count: usize,
    may_be_empty: bool,
    found_by_symbol: BTreeMap<String, Vec<(RealExchange, String)>>,
    found_by_exchange: BTreeMap<RealExchange, BTreeMap<String, BTreeSet<String>>>,
    skipped_by_exchange: BTreeMap<RealExchange, BTreeMap<String, BTreeSet<String>>>,
}

impl InstrumentTrace {
    pub fn new(name: &'static str, may_be_empty: bool) -> InstrumentTrace {
        trace!("Getting a list of available {name} from T-Bank...");

        InstrumentTrace {
            name,
            count: 0,
            may_be_empty,
            found_by_symbol: BTreeMap::new(),
            found_by_exchange: BTreeMap::new(),
            skipped_by_exchange: BTreeMap::new(),
        }
    }

    pub fn found(&mut self, real_exchange: RealExchange, exchange: String, symbol: String) {
        self.count += 1;

        if log_enabled!(Level::Trace) {
            self.found_by_symbol.entry(symbol.clone()).or_default()
                .push((real_exchange, exchange.clone()));

            self.found_by_exchange.entry(real_exchange).or_default()
                .entry(exchange).or_default()
                .insert(symbol);
        }
    }

    pub fn skipped(&mut self, real_exchange: RealExchange, exchange: String, symbol: String) {
        if log_enabled!(Level::Trace) {
            self.skipped_by_exchange.entry(real_exchange).or_default()
                .entry(exchange).or_default()
                .insert(symbol);
        }
    }

    pub fn finish(self) -> EmptyResult {
        if log_enabled!(Level::Trace) {
            trace!("Got the following {} from T-Bank:", self.name);
            for (real_exchange, exchanges) in self.found_by_exchange {
                trace!("* {}:", real_exchange.as_str_name());
                for (exchange, symbols) in exchanges {
                    trace!("  * {}: {}", exchange, symbols.iter().join(", "));
                }
            }

            if !self.skipped_by_exchange.is_empty() {
                trace!("Skipped non-{} from T-Bank:", self.name);
                for (real_exchange, exchanges) in self.skipped_by_exchange {
                    trace!("* {}:", real_exchange.as_str_name());
                    for (exchange, symbols) in exchanges {
                        trace!("  * {}: {}", exchange, symbols.iter().join(", "));
                    }
                }
            }

            let mut has_duplicates = false;
            for (symbol, exchanges) in self.found_by_symbol {
                if exchanges.len() == 1 {
                    continue;
                }

                if !has_duplicates {
                    trace!("Duplicated {} from T-Bank:", self.name);
                    has_duplicates = true;
                }

                trace!("* {}: {}", symbol, exchanges.iter().map(|(real_exchange, exchange)| {
                    format!("{}/{}", real_exchange.as_str_name(), exchange)
                }).join(", "));
            }
        }

        if !self.may_be_empty && self.count == 0 {
            return Err!("Got an empty list of available {}", self.name);
        }

        Ok(())
    }
}