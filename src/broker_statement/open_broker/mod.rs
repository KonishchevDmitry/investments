use std::fs::File;
use std::io::{Read, BufReader};

use chrono::Duration;
use encoding_rs;
use serde_xml_rs;

use brokers::{self, BrokerInfo};
#[cfg(test)] use config::{Config, Broker};
use config::BrokerConfig;
use core::GenericResult;
use currency::{Cash, CashAssets};

use super::{BrokerStatement, BrokerStatementReader, BrokerStatementBuilder};

use self::model::BrokerReport;

mod parsers;
mod model;

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

// FIXME: Deprecate
pub struct StatementParser {
    statement: BrokerStatementBuilder,
    currency: &'static str,
}

impl StatementParser {
    fn parse(mut self, path: &str) -> GenericResult<BrokerStatement> {
        let statement = read_statement(path)?;
        statement.parse(&mut self.statement)?;

        // FIXME: HERE
        self.statement.deposits.push(CashAssets::new_from_cash(date!(1, 1, 2017), Cash::new("RUB", dec!(1))));

        self.statement.get()
    }

}

fn read_statement(path: &str) -> GenericResult<BrokerReport> {
    let mut data = Vec::new();

    let mut reader = BufReader::new(File::open(path)?);
    reader.read_to_end(&mut data)?;

    let (data, _, errors) = encoding_rs::WINDOWS_1251.decode(data.as_slice());
    if errors {
        return Err!("Got an invalid Windows-1251 encoded data");
    }

    Ok(serde_xml_rs::from_str(&data).map_err(|e| e.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing() {
        let statement = BrokerStatement::read(
            &Config::mock(), Broker::OpenBroker, "testdata/open-broker").unwrap();

        // TODO: More checks
        assert!(statement.deposits.len() > 0);
    }
}