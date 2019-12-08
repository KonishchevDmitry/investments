// FIXME: All below

mod common;
mod parser;
mod parsers;

use crate::brokers::{Broker, BrokerInfo};
use crate::config::Config;
use crate::core::GenericResult;

#[cfg(test)] use super::{BrokerStatement};
use super::{BrokerStatementReader, PartialBrokerStatement};

use parser::{Parser, SectionParser};

pub struct StatementReader {
    broker_info: BrokerInfo,
}

impl StatementReader {
    pub fn new(config: &Config) -> GenericResult<Box<dyn BrokerStatementReader>> {
        Ok(Box::new(StatementReader {
            broker_info: Broker::OpenBroker.get_info(config)?,
        }))
    }
}

impl BrokerStatementReader for StatementReader {
    fn is_statement(&self, file_name: &str) -> bool {
        file_name.ends_with(".xls")
    }

    #[allow(unreachable_code)] // FIXME
    fn read(&self, path: &str) -> GenericResult<PartialBrokerStatement> {
        parser::read_statement(path)?;
        unimplemented!();
        let mut statement = PartialBrokerStatement::new(self.broker_info.clone());
        statement.validate()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_real() {
        // FIXME

        ::easy_logging::init(module_path!().split("::").next().unwrap(), ::log::Level::Trace).unwrap();

        let statement = BrokerStatement::read(
            &Config::mock(), Broker::Bcs, "/Users/konishchev/Cloud/Archive/Brokerage/БКС").unwrap();

        assert!(!statement.cash_flows.is_empty());
        assert!(!statement.cash_assets.is_empty());
        assert!(statement.idle_cash_interest.is_empty());

        assert!(!statement.stock_buys.is_empty());
        assert!(!statement.stock_sells.is_empty());
        assert!(statement.dividends.is_empty());

        assert!(!statement.open_positions.is_empty());
        assert!(!statement.instrument_names.is_empty());
    }
}