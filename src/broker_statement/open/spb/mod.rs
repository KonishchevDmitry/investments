use serde::Deserialize;

use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::GenericResult;
// use crate::types::Date;

// use common::deserialize_date;

#[derive(Deserialize)]
pub struct BrokerReport {
    // #[serde(deserialize_with = "deserialize_date")]
    // date_from: Date,

    // #[serde(deserialize_with = "deserialize_date")]
    // date_to: Date,
}

impl BrokerReport {
    pub fn parse(&self) -> GenericResult<PartialBrokerStatement> {
        let statement = PartialBrokerStatement::new(true);
        // statement.period = Some((self.date_from, self.date_to.succ()));
        Ok(statement)
    }
}