mod cash_assets;
mod common;
mod open_positions;
mod trades;

use serde::Deserialize;

use crate::broker_statement::open::common::deserialize_date;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::GenericResult;
use crate::types::Date;

use cash_assets::CashAssets;
use open_positions::OpenPositions;
use trades::Trades;

#[derive(Deserialize)]
pub struct BrokerReport {
    #[serde(deserialize_with = "deserialize_date")]
    date_from: Date,
    #[serde(deserialize_with = "deserialize_date")]
    date_to: Date,
    #[serde(rename = "account_totally_line")]
    cash_assets: CashAssets,
    #[serde(rename = "briefcase_position")]
    open_positions: OpenPositions,
    #[serde(rename = "closed_deal")]
    trades: Trades,
}

impl BrokerReport {
    pub fn parse(&self) -> GenericResult<PartialBrokerStatement> {
        let mut statement = PartialBrokerStatement::new(true);
        statement.period = Some((self.date_from, self.date_to.succ()));

        let mut has_starting_assets = self.cash_assets.parse(&mut statement)?;
        has_starting_assets |= self.open_positions.parse(&mut statement)?;
        statement.set_has_starting_assets(has_starting_assets)?;

        self.trades.parse(&mut statement)?;
        Ok(statement)
    }
}