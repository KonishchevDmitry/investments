mod balance;
mod common;
mod parser;
mod security_info;

use std::convert::TryInto;
use std::fs::File;
use std::io::{Read, BufReader, BufRead, Seek, SeekFrom};

#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::GenericResult;
#[cfg(test)] use crate::taxes::TaxRemapping;

#[cfg(test)] use super::{BrokerStatement};
use super::{BrokerStatementReader, PartialBrokerStatement};

use self::parser::OFX;

pub struct StatementReader {
}

impl StatementReader {
    pub fn new() -> GenericResult<Box<dyn BrokerStatementReader>> {
        Ok(Box::new(StatementReader{}))
    }
}

impl BrokerStatementReader for StatementReader {
    fn is_statement(&self, path: &str) -> GenericResult<bool> {
        Ok(path.ends_with(".ofx"))
    }

    // FIXME(konishchev): Implement
    fn read(&mut self, path: &str) -> GenericResult<PartialBrokerStatement> {
        read_statement(path)?.parse()
    }
}

fn read_statement(path: &str) -> GenericResult<OFX> {
    let file = File::open(path)?;
    let size: i64 = file.metadata()?.len().try_into().unwrap();
    let mut reader = BufReader::new(file);

    let mut header = String::new();
    reader.read_line(&mut header)?;
    if !header.starts_with("OFXHEADER:") {
        return Err!("Got an unexpected OFX file contents: OFXHEADER is missing");
    }

    loop {
        header.clear();

        if reader.read_line(&mut header)? == 0 {
            return Err!("Got an unexpected end of OFX file");
        }

        if header.trim_end_matches(|c| c == '\r' || c == '\n').is_empty() {
            break;
        }
    }

    let cur_pos: i64 = reader.seek(SeekFrom::Current(0))?.try_into().unwrap();
    let mut data = String::with_capacity(std::cmp::max(0, size - cur_pos).try_into().unwrap());

    reader.read_to_string(&mut data)?;
    if !data.starts_with("<OFX") {
        return Err!("Got an unexpected OFX file contents");
    }

    Ok(quick_xml::de::from_str(&data)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_real() {
        let broker = Broker::Firstrade.get_info(&Config::mock(), None).unwrap();

        let statement = BrokerStatement::read(
            broker, "testdata/firstrade", TaxRemapping::new(), true).unwrap();

        assert!(statement.cash_flows.is_empty());
        assert!(!statement.cash_assets.is_empty());

        assert!(statement.fees.is_empty());
        assert!(statement.idle_cash_interest.is_empty());

        assert!(statement.forex_trades.is_empty());
        assert!(statement.stock_buys.is_empty());
        assert!(statement.stock_sells.is_empty());
        assert!(statement.dividends.is_empty());

        assert!(statement.open_positions.is_empty());
        assert!(statement.instrument_names.is_empty());
    }
}