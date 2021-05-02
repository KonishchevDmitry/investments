use std::collections::{HashMap, hash_map::Entry};
use std::fmt;

use log::{debug, warn, error};

use crate::broker_statement::PartialBrokerStatement;
use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::formatting;
use crate::time::Date;

// Operation dates in broker statement sometimes differ from actual dates of cash flow operations on
// broker account. This helper provides the actual dates of cash flow operations.
pub struct CashFlows {
    cash_flows: HashMap<CashFlowId, CashFlowRecords>,
    enable_warnings: bool,
}

impl CashFlows {
    pub fn new(enable_warnings: bool) -> CashFlows {
        CashFlows {
            cash_flows: HashMap::new(),
            enable_warnings,
        }
    }

    pub fn add(&mut self, id: CashFlowId, date: Date) {
        self.cash_flows.entry(id)
            .or_insert_with(|| CashFlowRecords {dates: Vec::with_capacity(1), consumed: false})
            .dates.push(date);
    }

    pub fn map(&mut self, statement: &PartialBrokerStatement, id: CashFlowId, mut fallback: Date) -> GenericResult<Date> {
        if self.cash_flows.is_empty() {
            if self.enable_warnings {
                // https://github.com/KonishchevDmitry/investments/blob/master/docs/brokers.md#ib-cash-flow-info
                let url = "http://bit.ly/investments-ib-cash-flow-info";
                warn!(concat!(
                    "The broker statement misses account cash flow info (see {}). ",
                    "Operation dates may not be correct enough for account cash flow calculations.",
                ), url);
                self.enable_warnings = false;
            }
        } else {
            if let Entry::Occupied(mut entry) = self.cash_flows.entry(id.clone()) {
                let records = entry.get_mut();
                if let Some(&date) = records.dates.first() {
                    if date != id.statement_date {
                        debug!("{} is mapped to {}.", id, formatting::format_date(date));
                    }
                    records.dates.remove(0);
                    records.consumed = true;
                    return Ok(date);
                }
            };

            self.on_mapping_error(&id)?;
        }

        let period = statement.get_period()?;
        if fallback < period.0 {
            // FIXME(konishchev): Docs
            fallback = period.0;
        }

        if fallback != id.statement_date {
            debug!("{} is mapped to {}.", id, formatting::format_date(fallback));
        }
        Ok(fallback)
    }

    pub fn commit(mut self) -> GenericResult<bool> {
        let cash_flows = std::mem::take(&mut self.cash_flows);

        for (id, records) in cash_flows {
            if records.consumed && !records.dates.is_empty() {
                self.on_mapping_error(&id)?;
            }
        }

        Ok(self.enable_warnings)
    }

    fn on_mapping_error(&mut self, id: &CashFlowId) -> EmptyResult {
        let error = format!(concat!(
            "Not all operations were mapped to their cash flow dates correctly. ",
            "The first one is: {}",
        ), id);

        if cfg!(debug_assertions) {
            return Err(error.into());
        }

        if self.enable_warnings {
            error!("{}.", error);
            self.enable_warnings = false;
        }

        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CashFlowId {
    statement_date: Date,
    description: String,
    amount: Cash,
}

impl CashFlowId {
    pub fn new(statement_date: Date, description: &str, amount: Cash) -> CashFlowId {
        CashFlowId {statement_date, description: description.to_owned(), amount}
    }
}

impl fmt::Display for CashFlowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} / {} / {}", formatting::format_date(self.statement_date), self.description, self.amount)
    }
}

struct CashFlowRecords {
    dates: Vec<Date>,
    consumed: bool,
}