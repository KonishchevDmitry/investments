/*
Tinkoff statement T+N specific:

All trades are split into two categories: executed and not executed (yet). If trade is not executed
yet, it's listed in not executed table in current statement and then will be listed in executed
table of next statement, so we have to deduplicate them there (we want to capture T+N state of the
broker statement, because it's much intuitive and convenient compared to T+0 state).

Trade related cash flows are also listed in cash flow table. For not executed trades they have
"План" comment (and so may appear in several statements). At this time we ignore them because there
is no any new information inside of them. Dates there show that commission withholding is actually
T+N instead of expected T+0, but we don't take it into account yet.

Deposits and withholdings are always T+0, so there is no problems with them.
*/

mod assets;
mod cash_assets;
mod common;
mod foreign_income;
mod period;
mod securities;
mod trades;

use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use itertools::Itertools;
use lazy_static::lazy_static;
use regex::{self, Regex};

use crate::broker_statement::cash_flows::CashFlowType;
use crate::broker_statement::dividends::{DividendId, DividendAccruals};
use crate::broker_statement::taxes::{TaxId, TaxAccruals};
#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::{GenericResult, EmptyResult, GenericError};
use crate::exchanges::Exchange;
use crate::formats::xls::{XlsStatementParser, Section, SheetParser, SectionParserRc, Cell};
use crate::formatting;
use crate::instruments::{InstrumentId, parse_isin};
#[cfg(test)] use crate::taxes::TaxRemapping;

#[cfg(test)] use super::{BrokerStatement, ReadingStrictness};
use super::{BrokerStatementReader, PartialBrokerStatement};

use assets::AssetsParser;
use cash_assets::CashAssetsParser;
use common::SecuritiesRegistryRc;
use foreign_income::ForeignIncomeStatementReader;
use period::PeriodParser;
use securities::SecuritiesInfoParser;
use trades::{TradesParser, TradesRegistryRc};

pub struct StatementReader {
    trades: TradesRegistryRc,
    foreign_income: HashMap<DividendId, (DividendAccruals, TaxAccruals)>,
    show_missing_foreign_income_info_warning: bool,
}

impl StatementReader {
    pub fn new() -> GenericResult<Box<dyn BrokerStatementReader>> {
        Ok(Box::new(StatementReader{
            trades: TradesRegistryRc::default(),
            foreign_income: HashMap::new(),
            show_missing_foreign_income_info_warning: true,
        }))
    }

    fn parse_foreign_income_statement(&mut self, path: &str) -> EmptyResult {
        for (dividend_id, details) in ForeignIncomeStatementReader::read(path)? {
            if self.foreign_income.insert(dividend_id.clone(), details).is_some() {
                return Err!(
                    "Got a duplicated {}/{} dividend from different foreign income statements",
                    formatting::format_date(dividend_id.date), dividend_id.issuer);
            }
        }

        Ok(())
    }

    fn postprocess(&mut self, mut statement: PartialBrokerStatement) -> GenericResult<PartialBrokerStatement> {
        let mut dividends = HashMap::new();
        let mut taxes = HashMap::new();

        for trade in &mut statement.stock_buys {
            if let Ok(isin) = parse_isin(&trade.symbol) {
                let instrument = statement.instrument_info.get_by_id(&InstrumentId::Isin(isin)).map_err(|e| format!(
                    "Failed to remap {} trade from ISIN to stock symbol: {}", trade.symbol, e))?;
                trade.original_symbol = instrument.symbol.clone();
                trade.symbol = instrument.symbol.clone();
            }
        }

        for trade in &mut statement.stock_sells {
            if let Ok(isin) = parse_isin(&trade.symbol) {
                let instrument = statement.instrument_info.get_by_id(&InstrumentId::Isin(isin)).map_err(|e| format!(
                    "Failed to remap {} trade from ISIN to stock symbol: {}", trade.symbol, e))?;
                trade.original_symbol = instrument.symbol.clone();
                trade.symbol = instrument.symbol.clone();
            }
        }

        for cash_flow in &mut statement.cash_flows {
            match &mut cash_flow.type_ {
                CashFlowType::Repo {symbol, ..} => {
                    if let Ok(isin) = parse_isin(symbol) {
                        let instrument = statement.instrument_info.get_by_id(&InstrumentId::Isin(isin)).map_err(|e| format!(
                            "Failed to remap {} trade from ISIN to stock symbol: {}", symbol, e))?;
                        *symbol = instrument.symbol.clone();
                    }
                },
                CashFlowType::Dividend {..} | CashFlowType::Tax {..} => {
                    unreachable!();
                },
            }
        }

        for symbol in statement.open_positions.keys().cloned().collect::<Vec<String>>() {
            if let Ok(isin) = parse_isin(&symbol) {
                let map_err = |e: GenericError| -> GenericError {
                    format!("Failed to remap {} open position from ISIN to stock symbol: {}", symbol, e).into()
                };

                let new_symbol = statement.instrument_info.get_by_id(&InstrumentId::Isin(isin)).map_err(map_err)?.symbol.clone();
                let quantity = statement.open_positions.remove(&symbol).unwrap();
                statement.add_open_position(&new_symbol, quantity).map_err(map_err)?;
            }
        }

        for (mut dividend_id, dividend_accruals) in statement.dividend_accruals.drain().sorted_by_key(|(dividend_id, _)| {
            (dividend_id.date, dividend_id.issuer.to_string())
        }) {
            let instrument = match dividend_id.issuer {
                InstrumentId::Name(_) => {
                    statement.instrument_info.get_by_id(&dividend_id.issuer).map_err(|e| format!(
                        "Failed to process {}: {}", dividend_id.description(), e))?
                },
                _ => unreachable!(),
            };

            let mut tax_id = TaxId::new(dividend_id.date, dividend_id.issuer.clone());
            let tax_accruals = statement.tax_accruals.remove(&tax_id);

            let (dividend_accruals, tax_accruals) = foreign_income::match_statement_dividends_to_foreign_income(
                &dividend_id, instrument, dividend_accruals, tax_accruals,
                &mut self.foreign_income, &mut self.show_missing_foreign_income_info_warning)?;

            dividend_id.issuer = InstrumentId::Symbol(instrument.symbol.clone());
            assert!(dividends.insert(dividend_id, dividend_accruals).is_none());

            if let Some(tax_accruals) = tax_accruals {
                tax_id.issuer = InstrumentId::Symbol(instrument.symbol.clone());
                assert!(taxes.insert(tax_id, tax_accruals).is_none());
            }
        }

        if let Some(tax_id) = statement.tax_accruals.keys().next() {
            return Err!(
                "Got tax withholding for {} dividend from {} without the dividend itself",
                tax_id.issuer, formatting::format_date(tax_id.date));
        }

        statement.dividend_accruals = dividends;
        statement.tax_accruals = taxes;
        statement.validate()
    }
}

impl BrokerStatementReader for StatementReader {
    fn check(&mut self, path: &str) -> GenericResult<bool> {
        let is_foreign_income_statement = ForeignIncomeStatementReader::is_statement(path).map_err(|e| format!(
            "Error while reading {:?}: {}", path, e))?;

        if is_foreign_income_statement {
            self.parse_foreign_income_statement(path).map_err(|e| format!(
                "Error while reading {:?} foreign income statement: {}", path, e))?;
            return Ok(false);
        }

        Ok(path.ends_with(".xlsx"))
    }

    fn read(&mut self, path: &str, _is_last: bool) -> GenericResult<PartialBrokerStatement> {
        let parser = Box::new(StatementSheetParser{});
        let statement = PartialBrokerStatement::new_rc(&[Exchange::Moex, Exchange::Spb], true);

        let period_parser: SectionParserRc = Rc::new(RefCell::new(
            PeriodParser::new(statement.clone())));

        let executed_trades_parser: SectionParserRc = Rc::new(RefCell::new(
            TradesParser::new(true, statement.clone(), self.trades.clone())));

        let pending_trades_parser: SectionParserRc = Rc::new(RefCell::new(
            TradesParser::new(false, statement.clone(), self.trades.clone())));

        let cash_assets_parser = CashAssetsParser::new(statement.clone());

        let securities = SecuritiesRegistryRc::default();
        let assets_parser = AssetsParser::new(statement.clone(), securities.clone());
        let securities_info_parser = SecuritiesInfoParser::new(statement.clone(), securities);

        XlsStatementParser::read(path, parser, vec![
            Section::new(PeriodParser::CALCULATION_DATE_PREFIX).by_prefix()
                .parser_rc(period_parser.clone()).required(),
            Section::new(PeriodParser::PERIOD_PREFIX).by_prefix()
                .parser_rc(period_parser).required(),
            Section::new("1.1 Информация о совершенных и исполненных сделках на конец отчетного периода")
                .parser_rc(executed_trades_parser).required(),
            Section::new("1.2 Информация о неисполненных сделках на конец отчетного периода")
                .parser_rc(pending_trades_parser).required(),
            Section::new("2. Операции с денежными средствами и драг. металлами")
                .alias("2. Операции с денежными средствами")
                .parser(cash_assets_parser).required(),
            Section::new("3.1 Движение по ценным бумагам инвестора")
                .alias("3. Движение финансовых активов инвестора")
                .parser(assets_parser).required(),
            Section::new("4.1 Информация о ценных бумагах")
                .parser(securities_info_parser).required(),
        ])?;

        self.postprocess(Rc::try_unwrap(statement).ok().unwrap().into_inner())
    }

    fn close(self: Box<Self>) -> EmptyResult {
        if let Some(dividend_id) = self.foreign_income.keys().next() {
            return Err!(
                "Unable to match {} from foreign income report to any dividend from broker statement",
                dividend_id.description(),
            )
        }
        Ok(())
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
        let statement = parse("main", "my");

        assert!(!statement.assets.cash.is_empty());
        assert!(statement.assets.other.is_none()); // TODO(konishchev): Get it from statements
        assert!(!statement.deposits_and_withdrawals.is_empty());

        assert!(!statement.fees.is_empty());
        assert!(statement.idle_cash_interest.is_empty());
        assert!(!statement.tax_agent_withholdings.is_empty());

        assert!(!statement.forex_trades.is_empty());
        assert!(!statement.stock_buys.is_empty());
        assert!(!statement.stock_sells.is_empty());
        assert!(!statement.dividends.is_empty());

        assert!(!statement.open_positions.is_empty());
        assert!(!statement.instrument_info.is_empty());
    }

    #[rstest(name => ["complex", "mixed-currency-trade"])]
    fn parse_real_other(name: &str) {
        let statement = parse("other", name);
        assert_eq!(!statement.dividends.is_empty(), name == "complex");
    }

    fn parse(namespace: &str, name: &str) -> BrokerStatement {
        let portfolio_name = match (namespace, name) {
            ("main", "my") => s!("tinkoff"),
            ("other", name) => format!("tinkoff-{}", name),
            _ => name.to_owned(),
        };

        let broker = Broker::Tinkoff.get_info(&Config::mock(), None).unwrap();
        let config = Config::load(&format!("testdata/configs/{}/config.yaml", namespace)).unwrap();
        let portfolio = config.get_portfolio(&portfolio_name).unwrap();

        BrokerStatement::read(
            broker, &format!("testdata/tinkoff/{}", name),
            &Default::default(), &Default::default(), &Default::default(),
            TaxRemapping::new(), &portfolio.corporate_actions, ReadingStrictness::all(),
        ).unwrap()
    }
}