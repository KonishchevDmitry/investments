use std::fs::File;
use std::io::{Read, BufReader};

use encoding_rs;
use serde_xml_rs;

use crate::brokers::{Broker, BrokerInfo};
use crate::config::Config;
use crate::core::GenericResult;

#[cfg(test)] use super::{BrokerStatement};
use super::{BrokerStatementReader, PartialBrokerStatement};

use self::model::BrokerReport;

mod parsers;
mod model;

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
    fn is_statement(&self, file_name: &str) -> GenericResult<bool> {
        Ok(file_name.ends_with(".xml"))
    }

    fn read(&self, path: &str) -> GenericResult<PartialBrokerStatement> {
        let mut statement = PartialBrokerStatement::new(self.broker_info.clone());
        read_statement(path)?.parse(&mut statement)?;
        statement.validate()
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
    fn parse_real() {
        let statement = BrokerStatement::read(
            &Config::mock(), Broker::OpenBroker, "testdata/open-broker").unwrap();

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