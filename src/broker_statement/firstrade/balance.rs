use num_traits::Zero;
use serde::Deserialize;

use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::types::Decimal;

use super::StatementParser;
use super::common::{Ignore, deserialize_decimal};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Balance {
    #[serde(rename = "AVAILCASH", deserialize_with = "deserialize_decimal")]
    cash: Decimal,
    #[serde(rename = "MARGINBALANCE", deserialize_with = "deserialize_decimal")]
    margin: Decimal,
    #[serde(rename = "SHORTBALANCE", deserialize_with = "deserialize_decimal")]
    short: Decimal,
    #[serde(rename = "BUYPOWER")]
    _buy_power: Ignore,
    #[serde(rename = "BALLIST")]
    _other: Ignore,
}

impl Balance {
    pub fn parse(self, parser: &mut StatementParser, currency: &str) -> EmptyResult {
        if !self.margin.is_zero() || !self.short.is_zero() {
            return Err!("Margin accounts are not supported");
        }

        parser.statement.cash_assets.deposit(Cash::new(currency, self.cash));

        Ok(())
    }
}