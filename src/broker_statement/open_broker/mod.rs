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

pub struct StatementParser {
    statement: BrokerStatementBuilder,
    currency: &'static str,
}

impl StatementParser {
    // FIXME: HERE
    fn parse(mut self, path: &str) -> GenericResult<BrokerStatement> {
        let mut data = Vec::new();

        let mut reader = BufReader::new(File::open(path)?);
        reader.read_to_end(&mut data)?;

        let (data, _, errors) = encoding_rs::WINDOWS_1251.decode(data.as_slice());
        if errors {
            return Err!("Got an invalid Windows-1251 encoded data");
        }

        let statement: BrokerReport = serde_xml_rs::deserialize(data.as_bytes())?;
        self.statement.period = Some((statement.date_from, statement.date_to + Duration::days(1)));
        self.statement.cash_assets.deposit(Cash::new(self.currency, dec!(1)));
        self.statement.starting_value = Some(Cash::new("RUB", dec!(0)));
        self.statement.deposits.push(CashAssets::new_from_cash(date!(1, 1, 2017), Cash::new("RUB", dec!(1))));
        self.statement.get()
    }
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