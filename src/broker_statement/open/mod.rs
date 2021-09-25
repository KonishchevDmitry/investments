mod moex;

#[cfg(test)] use crate::brokers::Broker;
#[cfg(test)] use crate::config::Config;
use crate::core::GenericResult;
use crate::instruments::InstrumentInternalIds;
#[cfg(test)] use crate::taxes::TaxRemapping;

#[cfg(test)] use super::{BrokerStatement, ReadingStrictness};
use super::{BrokerStatementReader, PartialBrokerStatement};

use moex::BrokerReport;

pub struct StatementReader<'a> {
    instrument_internal_ids: &'a InstrumentInternalIds,
}

impl<'a> StatementReader<'a> {
    pub fn new(instrument_internal_ids: &'a InstrumentInternalIds) -> GenericResult<Box<dyn BrokerStatementReader + 'a>> {
        Ok(Box::new(StatementReader{instrument_internal_ids}))
    }
}

impl<'a> BrokerStatementReader for StatementReader<'a> {
    fn is_statement(&self, path: &str) -> GenericResult<bool> {
        Ok(path.ends_with(".xml"))
    }

    fn read(&mut self, path: &str, _is_last: bool) -> GenericResult<PartialBrokerStatement> {
        let mut statement = PartialBrokerStatement::new(true);
        read_statement(path)?.parse(&mut statement, self.instrument_internal_ids)?;
        statement.validate()
    }
}

fn read_statement(path: &str) -> GenericResult<BrokerReport> {
    let data = std::fs::read(path)?;

    let (data, _, errors) = encoding_rs::WINDOWS_1251.decode(data.as_slice());
    if errors {
        return Err!("Got an invalid Windows-1251 encoded data");
    }

    Ok(serde_xml_rs::from_str(&data).map_err(|e| e.to_string())?)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(name => ["main/my", "main/iia", "other/first-iia-a", "other/inactive-with-forex"])]
    fn parse_real(name: &str) {
        let (namespace, name) = name.split_once('/').unwrap();
        let statement = parse(namespace, name);

        assert_eq!(statement.cash_assets.is_empty(), name == "inactive-with-forex");
        assert!(!statement.deposits_and_withdrawals.is_empty());

        assert_eq!(statement.fees.is_empty(), name == "iia");
        assert!(statement.idle_cash_interest.is_empty());
        assert!(statement.tax_agent_withholdings.is_empty());

        assert_eq!(statement.forex_trades.is_empty(), matches!(name, "iia" | "first-iia-a"));
        assert_eq!(statement.stock_buys.is_empty(), name == "inactive-with-forex");
        assert_eq!(statement.stock_sells.is_empty(), matches!(name, "iia" | "inactive-with-forex"));
        assert!(statement.dividends.is_empty());

        assert_eq!(statement.open_positions.is_empty(), name == "inactive-with-forex");
        assert_eq!(statement.instrument_info.is_empty(), name == "inactive-with-forex");
    }

    #[rstest(name => ["dividends/moex"])]
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