use encoding_rs::Encoding;
use serde::Deserialize;
use xml::reader::{EventReader, XmlEvent};

#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::GenericResult;
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
        let (encoding_name, report_type) = preprocess_statement(&data)?;

        let encoding = Encoding::for_label(encoding_name.as_bytes()).ok_or_else(|| format!(
            "Unsupported document encoding: {:?}", encoding_name))?;

        let (data, _, errors) = encoding.decode(data.as_slice());
        if errors {
            return Err!("Got an invalid {} encoded data", encoding_name);
        }

        let statement = match report_type.as_str() {
            "https://account.open-broker.ru/common/report/broker_report_spot.xsl" |
            "https://account.open-broker.ru/common/report/broker_report_unified.xsl" => {
                let report: moex::BrokerReport = serde_xml_rs::from_str(&data)?;
                report.parse()?
            },

            "https://account.open-broker.ru/common/report/broker_report_spb.xsl" => {
                let report: spb::BrokerReport = serde_xml_rs::from_str(&data)?;
                report.parse()?
            },

            _ => return Err!("Unsupported Open Broker report type: {}", report_type),
        };

        statement.validate()
    }
}

fn preprocess_statement(data: &[u8]) -> GenericResult<(String, String)> {
    let mut document_encoding = None;
    let reader = EventReader::new(data);

    for event in reader {
        match event? {
            XmlEvent::StartDocument {encoding, ..} => {
                document_encoding.replace(encoding);
            },

            XmlEvent::ProcessingInstruction {name, data} if name == "xml-stylesheet" => {
                let data = data.unwrap_or_default();

                #[derive(Deserialize)]
                pub struct XmlStylesheet {
                    href: String,
                }

                let to_decode = format!("<{} {}/>", name, data);
                let xml_stylesheet: XmlStylesheet = serde_xml_rs::from_str(&to_decode).map_err(|_| format!(
                    "Unexpected {} contents: {:?}", name, data))?;

                return Ok((document_encoding.unwrap(), xml_stylesheet.href))
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

    #[rstest(name => ["main/my", "main/iia", "other/first-iia-a", "other/inactive-with-forex"])]
    fn parse_real(name: &str) {
        let (namespace, name) = name.split_once('/').unwrap();
        let statement = parse(namespace, name);

        assert_eq!(statement.assets.cash.is_empty(), name == "inactive-with-forex");
        assert!(statement.assets.other.is_none()); // TODO(konishchev): Get it from statements
        assert!(!statement.deposits_and_withdrawals.is_empty());

        assert_eq!(statement.fees.is_empty(), name == "iia");
        assert!(statement.idle_cash_interest.is_empty());
        assert!(statement.tax_agent_withholdings.is_empty());

        assert_eq!(statement.forex_trades.is_empty(), matches!(name, "iia" | "first-iia-a"));
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
            ("main", "my") => s!("open"),
            ("other", name) => format!("open-{}", name.replace('/', "-")),
            _ => name.to_owned(),
        };

        let broker = Broker::Open.get_info(&Config::mock(), None).unwrap();
        let config = Config::load(&format!("testdata/configs/{}/config.yaml", namespace)).unwrap();
        let portfolio = config.get_portfolio(&portfolio_name).unwrap();

        BrokerStatement::read(
            broker, &format!("testdata/open-broker/{}", name),
            &Default::default(), &portfolio.instrument_internal_ids, &Default::default(),
            TaxRemapping::new(), &portfolio.corporate_actions, ReadingStrictness::all(),
        ).unwrap()
    }
}
