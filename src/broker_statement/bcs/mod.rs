mod assets;
mod cash_flow;
mod common;
mod period;
mod trades;

#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::GenericResult;
#[cfg(test)] use crate::taxes::TaxRemapping;
use crate::xls::SheetParser;

#[cfg(test)] use super::{BrokerStatement};
use super::{BrokerStatementReader, PartialBrokerStatement};
use super::xls::{XlsStatementParser, Section};

use assets::AssetsParser;
use cash_flow::CashFlowParser;
use period::PeriodParser;
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
        Ok(path.ends_with(".xls"))
    }

    fn read(&mut self, path: &str, _is_last: bool) -> GenericResult<PartialBrokerStatement> {
        let parser = Box::new(StatementSheetParser{});

        XlsStatementParser::read(path, parser, vec![
            Section::new("Период:").parser(Box::new(PeriodParser{})).required(),

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
            Section::new("Рубль").parser(Box::new(CashFlowParser{})),

            Section::new("2.1. Сделки:"),
            Section::new("Пай").parser(Box::new(TradesParser{})),
            Section::new("2.3. Незавершенные сделки"),

            Section::new("3. Активы:").required(),
            Section::new("Вид актива").parser(Box::new(AssetsParser{})).required(),
        ])
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
        let broker = Broker::Bcs.get_info(&Config::mock(), None).unwrap();

        let statement = BrokerStatement::read(
            broker, &format!("testdata/bcs/{}", name),
            &hashmap!{}, &hashmap!{}, TaxRemapping::new(), true).unwrap();

        assert!(!statement.cash_flows.is_empty());
        assert!(!statement.cash_assets.is_empty());

        assert!(!statement.fees.is_empty());
        assert!(statement.idle_cash_interest.is_empty());
        assert!(statement.tax_agent_withholdings.is_empty());

        assert!(statement.forex_trades.is_empty());
        assert!(!statement.stock_buys.is_empty());
        assert_eq!(statement.stock_sells.is_empty(), name == "my");
        assert!(statement.dividends.is_empty());

        assert!(!statement.open_positions.is_empty());
        assert!(statement.instrument_names.is_empty());
    }
}