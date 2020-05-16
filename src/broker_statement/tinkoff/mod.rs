mod cash_assets;
mod common;
mod period;

use std::cell::RefCell;
use std::rc::Rc;

use crate::brokers::{Broker, BrokerInfo};
use crate::config::Config;
use crate::core::GenericResult;
#[cfg(test)] use crate::taxes::TaxRemapping;

#[cfg(test)] use super::{BrokerStatement};
use super::{BrokerStatementReader, PartialBrokerStatement};
use super::xls::{XlsStatementParser, Section, SectionParserRc};

use cash_assets::CashFlowParser;
use period::PeriodParser;

pub struct StatementReader {
    broker_info: BrokerInfo,
}

impl StatementReader {
    pub fn new(config: &Config) -> GenericResult<Box<dyn BrokerStatementReader>> {
        Ok(Box::new(StatementReader {
            broker_info: Broker::Tinkoff.get_info(config)?,
        }))
    }
}

impl BrokerStatementReader for StatementReader {
    fn is_statement(&self, path: &str) -> GenericResult<bool> {
        Ok(path.ends_with(".xlsx"))
    }

    // FIXME(konishchev): Work in progress
    fn read(&mut self, path: &str) -> GenericResult<PartialBrokerStatement> {
        let period_parser: SectionParserRc = Rc::new(RefCell::new(Box::new(PeriodParser::default())));
        XlsStatementParser::read(self.broker_info.clone(), path, "broker_rep", vec![
            Section::new(PeriodParser::CALCULATION_DATE_PREFIX).by_prefix().parser_rc(period_parser.clone()).required(),
            Section::new(PeriodParser::PERIOD_PREFIX).by_prefix().parser_rc(period_parser).required(),
            Section::new("2. Операции с денежными средствами").parser(Box::new(CashFlowParser{})).required(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_real() {
        let statement = BrokerStatement::read(
            &Config::mock(), Broker::Tinkoff, "testdata/tinkoff", TaxRemapping::new(), true).unwrap();

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