mod balance;
mod common;
mod dividends;
mod open_positions;
mod parser;
mod security_info;
mod transactions;

use std::convert::TryInto;
use std::fs::File;
use std::io::{Read, BufReader, BufRead, Seek};

#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::GenericResult;
#[cfg(test)] use crate::taxes::TaxRemapping;

#[cfg(test)] use super::{BrokerStatement, ReadingStrictness};
use super::{BrokerStatementReader, PartialBrokerStatement};

use self::parser::{StatementParser, Ofx};

pub struct StatementReader {
    warn_on_missing_dividend_details: bool,
}

impl StatementReader {
    pub fn new() -> GenericResult<Box<dyn BrokerStatementReader>> {
        Ok(Box::new(StatementReader{
            warn_on_missing_dividend_details: true,
        }))
    }
}

impl BrokerStatementReader for StatementReader {
    fn check(&mut self, path: &str) -> GenericResult<bool> {
        Ok(path.ends_with(".ofx"))
    }

    fn read(&mut self, path: &str, is_last: bool) -> GenericResult<PartialBrokerStatement> {
        StatementParser::parse(self, read_statement(path)?, is_last)
    }
}

fn read_statement(path: &str) -> GenericResult<Ofx> {
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

        if header.trim_end_matches(['\r', '\n']).is_empty() {
            break;
        }
    }

    let cur_pos: i64 = reader.stream_position()?.try_into().unwrap();
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
            broker, "testdata/firstrade/my", &[], &Default::default(), &Default::default(), TaxRemapping::new(),
            &[], &[], ReadingStrictness::all()).unwrap();

        assert!(!statement.assets.cash.is_empty());
        assert!(statement.assets.other.is_none()); // TODO(konishchev): Get it from statements
        assert!(!statement.deposits_and_withdrawals.is_empty());

        assert!(!statement.fees.is_empty());
        assert!(statement.cash_grants.is_empty());
        assert!(!statement.idle_cash_interest.is_empty());
        assert!(statement.tax_agent_withholdings.is_empty());

        assert!(statement.forex_trades.is_empty());
        assert!(!statement.stock_buys.is_empty());
        assert!(!statement.stock_sells.is_empty());
        assert!(!statement.dividends.is_empty());

        assert!(statement.open_positions.is_empty());
        assert!(!statement.instrument_info.is_empty());
    }
}