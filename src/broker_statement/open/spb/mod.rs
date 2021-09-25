mod cash_assets;

use serde::Deserialize;

use crate::broker_statement::open::common::deserialize_date;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::GenericResult;
use crate::types::Date;

use cash_assets::AccountSummary;

#[derive(Deserialize)]
pub struct BrokerReport {
    #[serde(deserialize_with = "deserialize_date")]
    date_from: Date,

    #[serde(deserialize_with = "deserialize_date")]
    date_to: Date,

    #[serde(rename = "account_totally_line")]
    account_summary: AccountSummary,
}

impl BrokerReport {
    pub fn parse(&self) -> GenericResult<PartialBrokerStatement> {
        let mut statement = PartialBrokerStatement::new(true);
        statement.period = Some((self.date_from, self.date_to.succ()));

        self.account_summary.parse(&mut statement)?;

        // FIXME(konishchev): Support
        // statement.set_has_starting_assets(has_starting_assets)

        Ok(statement)
    }
}