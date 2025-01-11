mod assets;
mod cash_assets;
mod cash_flow;
mod common;
mod period;
mod securities;
mod trades;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use itertools::Itertools;

#[cfg(test)] use crate::broker_statement::{BrokerStatement, ReadingStrictness};
use crate::broker_statement::{BrokerStatementReader, PartialBrokerStatement};
#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::GenericResult;
use crate::exchanges::Exchange;
use crate::formats::html::{HtmlStatementParser, Section};
use crate::instruments::InstrumentId;
#[cfg(test)] use crate::taxes::TaxRemapping;

use assets::AssetsParser;
use cash_assets::CashAssetsParser;
use cash_flow::CashFlowParser;
use period::PeriodParser;
use securities::SecuritiesInfoParser;
use trades::TradesParser;

pub struct StatementReader {
    trades: Rc<RefCell<HashSet<u64>>>,
}

impl StatementReader {
    pub fn new() -> GenericResult<Box<dyn BrokerStatementReader>> {
        Ok(Box::new(StatementReader {
            trades: Default::default(),
        }))
    }
}

impl BrokerStatementReader for StatementReader {
    fn check(&mut self, path: &str) -> GenericResult<bool> {
        Ok(path.ends_with(".html"))
    }

    fn read(&mut self, path: &str, _is_last: bool) -> GenericResult<PartialBrokerStatement> {
        let statement = PartialBrokerStatement::new_rc(&[Exchange::Moex, Exchange::Spb], true);

        HtmlStatementParser::read(path, vec![
            Section::new("Отчет брокера за период").by_prefix().required().parser(PeriodParser::new(statement.clone())),
            Section::new("Портфель Ценных Бумаг").by_prefix().parser(AssetsParser::new(statement.clone())),
            Section::new("Денежные средства").required().parser(CashAssetsParser::new(statement.clone())),
            Section::new("Движение денежных средств за период").required().parser(CashFlowParser::new(statement.clone())),
            Section::new("Сделки купли/продажи ценных бумаг").parser(TradesParser::new(statement.clone(), self.trades.clone())),
            Section::new("Справочник Ценных Бумаг").parser(SecuritiesInfoParser::new(statement.clone())),
        ])?;

        let mut statement = Rc::try_unwrap(statement).ok().unwrap().into_inner();

        for (name, quantity) in statement.open_positions.drain().collect_vec() {
            let symbol = statement.instrument_info.get_by_id(&InstrumentId::Name(name)).map_err(|e| format!(
                "Open positions parser: {e}"))?.symbol.clone();
            statement.add_open_position(&symbol, quantity)?;
        }

        statement.validate()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(name => ["my", "iia"])]
    fn parse_real(name: &str) {
        let portfolio_name = match name {
            "my" => "sber",
            "iia" => "sber-iia",
            _ => name,
        };

        let path = format!("testdata/sber/{}", name);
        let broker = Broker::Sber.get_info(&Config::mock(), None).unwrap();
        let config = Config::load("testdata/configs/main/config.yaml").unwrap();
        let corporate_actions = &config.get_portfolio(portfolio_name).unwrap().corporate_actions;

        let statement = BrokerStatement::read(
            broker, &path, &Default::default(), &Default::default(), &Default::default(), TaxRemapping::new(), &[],
            corporate_actions, ReadingStrictness::all()).unwrap();

        assert_eq!(statement.assets.cash.is_empty(), name == "my");
        assert!(statement.assets.other.is_none()); // TODO(konishchev): Get it from statements
        assert!(!statement.deposits_and_withdrawals.is_empty());

        assert!(statement.fees.is_empty());
        assert_eq!(statement.cash_grants.is_empty(), name != "my");
        assert!(statement.idle_cash_interest.is_empty());
        assert!(statement.tax_agent_withholdings.is_empty());

        assert!(statement.forex_trades.is_empty());
        assert!(!statement.stock_buys.is_empty());
        assert!(statement.stock_sells.is_empty());
        assert!(statement.dividends.is_empty());

        assert!(!statement.open_positions.is_empty());
        assert!(!statement.instrument_info.is_empty());
    }
}