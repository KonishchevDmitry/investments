mod assets;
mod cash_flow;
mod common;
mod period;
mod securities;
mod trades;

use std::rc::Rc;

#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::GenericResult;
use crate::exchanges::Exchange;
#[cfg(test)] use crate::taxes::TaxRemapping;
use crate::xls::{XlsStatementParser, Section, SheetParser};

#[cfg(test)] use super::{BrokerStatement, ReadingStrictness};
use super::{BrokerStatementReader, PartialBrokerStatement};

use assets::AssetsParser;
use cash_flow::CashFlowParser;
use period::PeriodParser;
use securities::SecuritiesParser;
use trades::TradesParser;

pub struct StatementReader {
}

impl StatementReader {
    pub fn new() -> GenericResult<Box<dyn BrokerStatementReader>> {
        Ok(Box::new(StatementReader{}))
    }
}

impl BrokerStatementReader for StatementReader {
    fn check(&mut self, path: &str) -> GenericResult<bool> {
        Ok(path.ends_with(".xls"))
    }

    fn read(&mut self, path: &str, _is_last: bool) -> GenericResult<PartialBrokerStatement> {
        let parser = Box::new(StatementSheetParser{});
        let statement = PartialBrokerStatement::new_rc(&[Exchange::Moex, Exchange::Spb], true);

        XlsStatementParser::read(path, parser, vec![
            Section::new("Период:").parser(PeriodParser::new(statement.clone())).required(),

            Section::new("1. Движение денежных средств").required(),
            Section::new("1.1. Движение денежных средств по совершенным сделкам:").required(),
            Section::new(concat!(
                "1.1.1. Движение денежных средств по совершенным сделкам (иным операциям) с ",
                "ценными бумагами, по срочным сделкам, а также сделкам с иностранной валютой:",
            )).required(),
            Section::new("Остаток денежных средств на начало периода (Рубль):")
                .alias("Задолженность перед Компанией на начало периода (Рубль):").required(),
            Section::new("Остаток денежных средств на конец периода (Рубль):")
                .alias("Задолженность перед Компанией на конец периода (Рубль):").required(),
            Section::new("Рубль").parser(CashFlowParser::new(statement.clone())),

            Section::new("2.1. Сделки:"),
            Section::new("Пай").parser(TradesParser::new(statement.clone())),
            Section::new("2.3. Незавершенные сделки"),

            Section::new("3. Активы:").required(),
            Section::new("Вид актива").parser(AssetsParser::new(statement.clone())).required(),

            Section::new("4. Движение Ценных бумаг").parser(SecuritiesParser::new(statement.clone())),
        ])?;

        Rc::try_unwrap(statement).ok().unwrap().into_inner().validate()
    }
}

struct StatementSheetParser {
}

impl SheetParser for StatementSheetParser {
    fn sheet_name(&self) -> &str {
        "TDSheet"
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(name => ["my", "kate", "kate-iia"])]
    fn parse_real(name: &str) {
        let portfolio_name = match name {
            "my" => "bcs",
            _ => name,
        };

        let broker = Broker::Bcs.get_info(&Config::mock(), None).unwrap();
        let config = Config::load("testdata/configs/main/config.yaml").unwrap();
        let corporate_actions = &config.get_portfolio(portfolio_name).unwrap().corporate_actions;

        let statement = BrokerStatement::read(
            broker, &format!("testdata/bcs/{}", name),
            &Default::default(), &Default::default(), &Default::default(), TaxRemapping::new(),
            corporate_actions, ReadingStrictness::all()).unwrap();

        assert!(!statement.cash_assets.is_empty());
        assert!(!statement.deposits_and_withdrawals.is_empty());

        assert!(!statement.fees.is_empty());
        assert!(statement.idle_cash_interest.is_empty());
        assert_eq!(statement.tax_agent_withholdings.is_empty(), name != "kate");

        assert!(statement.forex_trades.is_empty());
        assert!(!statement.stock_buys.is_empty());
        assert!(!statement.stock_sells.is_empty());
        assert!(statement.dividends.is_empty());

        assert!(!statement.open_positions.is_empty());
        assert!(!statement.instrument_info.is_empty());
    }
}