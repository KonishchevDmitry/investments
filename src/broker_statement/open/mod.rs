use serde::Deserialize;
use ::xml::reader::{ParserConfig, EventReader, XmlEvent};

#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::GenericResult;
use crate::formats::xml;
#[cfg(test)] use crate::taxes::TaxRemapping;

#[cfg(test)] use super::{BrokerStatement, ReadingStrictness};
use super::{BrokerStatementReader, PartialBrokerStatement};

mod common;
mod moex;
mod spb;

pub struct StatementReader {
}

impl StatementReader {
    pub fn new() -> GenericResult<Box<dyn BrokerStatementReader>> {
        Ok(Box::new(StatementReader{}))
    }
}

impl BrokerStatementReader for StatementReader {
    fn check(&mut self, path: &str) -> GenericResult<bool> {
        Ok(path.ends_with(".xml"))
    }

    fn read(&mut self, path: &str, _is_last: bool) -> GenericResult<PartialBrokerStatement> {
        let data = std::fs::read(path)?;
        let report_type = get_report_type(&data)?;

        let statement = match report_type.as_str() {
            "https://account.open-broker.ru/common/report/broker_report_spot.xsl" |
            "https://account.open-broker.ru/common/report/broker_report_unified.xsl" => {
                let report: moex::BrokerReport = xml::deserialize(data.as_slice())?;
                report.parse()?
            },

            "https://account.open-broker.ru/common/report/broker_report_spb.xsl" => {
                let report: spb::BrokerReport = xml::deserialize(data.as_slice())?;
                report.parse()?
            },

            _ => return Err!("Unsupported Open Broker report type: {}", report_type),
        };

        statement.validate()
    }
}

fn get_report_type(data: &[u8]) -> GenericResult<String> {
    let config = ParserConfig::new().ignore_invalid_encoding_declarations(true);

    for event in EventReader::new_with_config(data, config) {
        match event? {
            XmlEvent::ProcessingInstruction {name, data} if name == "xml-stylesheet" => {
                let data = data.unwrap_or_default()
                    // xml-rs has some strange bug here which converts "/" to "</". Workaround it.
                    .replace("</", "/");

                #[derive(Deserialize)]
                pub struct XmlStylesheet {
                    #[serde(rename = "@href")]
                    href: String,
                }

                let to_decode = format!("<{} {}/>", name, data);
                let xml_stylesheet: XmlStylesheet = xml::deserialize(to_decode.as_bytes()).map_err(|_| format!(
                    "Unexpected {name} contents: {data:?}"))?;

                return Ok(xml_stylesheet.href)
            },

            _ => {},
        }
    }

    Err!("The statement file has an unexpected contents")
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest(name => ["main/my", "other/iia-a", "other/iia-b", "other/inactive-with-forex"])]
    fn parse_real(name: &str) {
        let (namespace, name) = name.split_once('/').unwrap();
        let statement = parse(namespace, name);

        assert_eq!(statement.assets.cash.is_empty(), name == "inactive-with-forex");
        assert!(statement.assets.other.is_none()); // TODO(konishchev): Get it from statements
        assert!(!statement.deposits_and_withdrawals.is_empty());

        assert_eq!(statement.fees.is_empty(), name == "iia-b");
        assert!(statement.cash_grants.is_empty());
        assert!(statement.idle_cash_interest.is_empty());
        assert_eq!(statement.tax_agent_withholdings.is_empty(), name != "my");

        assert_eq!(statement.forex_trades.is_empty(), matches!(name, "iia-a" | "iia-b"));
        assert_eq!(statement.stock_buys.is_empty(), name == "inactive-with-forex");
        assert_eq!(statement.stock_sells.is_empty(), name == "inactive-with-forex");
        assert!(statement.dividends.is_empty());

        assert_eq!(statement.open_positions.is_empty(), name == "inactive-with-forex");
        assert_eq!(statement.instrument_info.is_empty(), name == "inactive-with-forex");
    }

    #[rstest(name => ["dividends/moex", "dividends/spb"])]
    fn parse_real_dividends(name: &str) {
        let statement = parse("other", name);
        assert!(!statement.dividends.is_empty());
    }

    fn parse(namespace: &str, name: &str) -> BrokerStatement {
        let portfolio_name = match (namespace, name) {
            ("main", "my") => s!("investpalata"),
            ("other", name) => format!("open-{}", name.replace('/', "-")),
            _ => name.to_owned(),
        };

        let broker = Broker::Open.get_info(&Config::mock(), None).unwrap();
        let config = Config::new(format!("testdata/configs/{}", namespace), None).unwrap();
        let portfolio = config.get_portfolio(&portfolio_name).unwrap();

        BrokerStatement::read(
            broker, &format!("testdata/open/{}", name),
            &Default::default(), &portfolio.instrument_internal_ids, &Default::default(), TaxRemapping::new(), &[],
            &portfolio.corporate_actions, ReadingStrictness::all(),
        ).unwrap()
    }
}