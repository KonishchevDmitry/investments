mod assets;
mod cash_assets;
mod common;
mod period;
mod securities;
mod trades;

use std::cell::RefCell;
use std::rc::Rc;

use lazy_static::lazy_static;
use regex::{self, Regex};

#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::GenericResult;
#[cfg(test)] use crate::taxes::TaxRemapping;
use crate::xls::{SheetParser, Cell};

#[cfg(test)] use super::{BrokerStatement, ReadingStrictness};
use super::{BrokerStatementReader, PartialBrokerStatement};
use super::xls::{XlsStatementParser, Section, SectionParserRc};

use assets::AssetsParser;
use cash_assets::CashAssetsParser;
use period::PeriodParser;
use securities::SecuritiesInfoParser;
use trades::TradesParser;

pub struct StatementReader {
}

impl StatementReader {
    pub fn new() -> GenericResult<Box<dyn BrokerStatementReader>> {
        Ok(Box::new(StatementReader{}))
    }
}

impl BrokerStatementReader for StatementReader {
    fn is_statement(&self, path: &str) -> GenericResult<bool> {
        Ok(path.ends_with(".xlsx"))
    }

    fn read(&mut self, path: &str, _is_last: bool) -> GenericResult<PartialBrokerStatement> {
        let sheet_parser = Box::new(StatementSheetParser{});
        let period_parser: SectionParserRc = Rc::new(RefCell::new(Box::new(PeriodParser::default())));

        let mut statement = XlsStatementParser::read(path, sheet_parser, vec![
            Section::new(PeriodParser::CALCULATION_DATE_PREFIX)
                .by_prefix().parser_rc(period_parser.clone()).required(),
            Section::new(PeriodParser::PERIOD_PREFIX)
                .by_prefix().parser_rc(period_parser).required(),
            Section::new("1.1 Информация о совершенных и исполненных сделках на конец отчетного периода")
                .parser(Box::new(TradesParser {})).required(),
            Section::new("1.2 Информация о неисполненных сделках на конец отчетного периода")
                .parser(Box::new(TradesParser {})).required(),
            Section::new("2. Операции с денежными средствами")
                .parser(Box::new(CashAssetsParser {})).required(),
            Section::new("3.1 Движение по ценным бумагам инвестора")
                .alias("3. Движение финансовых активов инвестора")
                .parser(Box::new(AssetsParser {})).required(),
            Section::new("4.1 Информация о ценных бумагах")
                .parser(Box::new(SecuritiesInfoParser {})).required(),
        ])?;

        cash_assets::postprocess(&mut statement)?;
        Ok(statement)
    }
}

struct StatementSheetParser {
}

impl SheetParser for StatementSheetParser {
    fn sheet_name(&self) -> &str {
        "broker_rep"
    }

    fn repeatable_table_column_titles(&self) -> bool {
        true
    }

    fn skip_row(&self, row: &[Cell]) -> bool {
        lazy_static! {
            static ref CURRENT_PAGE_REGEX: Regex = Regex::new(r"^\d+ из$").unwrap();
        }

        enum State {
            None,
            CurrentPage,
            TotalPages,
        }
        let mut state = State::None;

        for cell in row {
            match cell {
                Cell::Empty => {},
                Cell::String(value) => {
                    if !matches!(state, State::None) || !CURRENT_PAGE_REGEX.is_match(value.trim()) {
                        return false;
                    }
                    state = State::CurrentPage;
                }
                Cell::Float(_) | Cell::Int(_) => {
                    if !matches!(state, State::CurrentPage) {
                        return false;
                    }
                    state = State::TotalPages;
                }
                _ => return false,
            };
        }

        matches!(state, State::TotalPages)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[test]
    fn parse_real() {
        let statement = parse("my");

        assert!(!statement.cash_assets.is_empty());
        assert!(!statement.deposits_and_withdrawals.is_empty());

        assert!(!statement.fees.is_empty());
        assert!(statement.idle_cash_interest.is_empty());
        assert!(statement.tax_agent_withholdings.is_empty());

        assert!(!statement.forex_trades.is_empty());
        assert!(!statement.stock_buys.is_empty());
        assert!(!statement.stock_sells.is_empty());
        assert!(statement.dividends.is_empty());

        assert!(!statement.open_positions.is_empty());
        assert!(!statement.instrument_names.is_empty());
    }

    #[rstest(name => ["mixed-currency-trade"])]
    fn parse_real_other(name: &str) {
        parse(name);
    }

    fn parse(name: &str) -> BrokerStatement {
        let broker = Broker::Tinkoff.get_info(&Config::mock(), None).unwrap();
        let path = format!("testdata/tinkoff/{}", name);
        BrokerStatement::read(
            broker, &path, &hashmap!{}, &hashmap!{}, TaxRemapping::new(), &[],
            ReadingStrictness::all()).unwrap()
    }
}