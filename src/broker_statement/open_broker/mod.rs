use std::collections::HashMap;
use std::iter::Iterator;

use brokers::{self, BrokerInfo};
#[cfg(test)] use config::{Config, Broker};
use config::BrokerConfig;
use core::GenericResult;
use currency::Cash;

use super::{BrokerStatement, BrokerStatementReader, BrokerStatementBuilder};

pub struct StatementReader {
    broker_info: BrokerInfo,
}

impl StatementReader {
    pub fn new(config: &BrokerConfig) -> Box<BrokerStatementReader> {
        Box::new(StatementReader {
            broker_info: brokers::open_broker(config),
        })
    }
}

impl BrokerStatementReader for StatementReader {
    fn is_statement(&self, file_name: &str) -> bool {
        file_name.ends_with(".xml")
    }

    fn read(&self, path: &str) -> GenericResult<BrokerStatement> {
        let parser = StatementParser {
            statement: BrokerStatementBuilder::new(self.broker_info.clone()),
            currency: "RUB", // TODO: Get from statement
            // FIXME: Taxes, dividends
        };

        parser.parse(path)
    }
}

pub struct StatementParser {
    statement: BrokerStatementBuilder,
    currency: &'static str,
}

impl StatementParser {
    fn parse(mut self, path: &str) -> GenericResult<BrokerStatement> {
        unreachable!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FIXME
    /*
    #[test]
    fn parsing() {
        let statement = BrokerStatement::read(
            &Config::mock(), Broker::OpenBroker, "testdata/open-broker").unwrap();

        // TODO: More checks
        assert!(statement.deposits.len() > 0);
    }
    */
}