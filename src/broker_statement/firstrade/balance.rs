use num_traits::Zero;
use serde::Deserialize;

use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::types::Decimal;
use crate::util::{DecimalRestrictions, validate_decimal};

use super::common::{Ignore, deserialize_decimal};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Balance {
    #[serde(rename = "AVAILCASH", deserialize_with = "deserialize_decimal")]
    cash: Decimal,
    #[serde(rename = "MARGINBALANCE", deserialize_with = "deserialize_decimal")]
    margin: Decimal,
    #[serde(rename = "SHORTBALANCE", deserialize_with = "deserialize_decimal")]
    short: Decimal,
    #[serde(rename = "BUYPOWER")]
    buy_power: Ignore,
    #[serde(rename = "BALLIST")]
    other: Ignore,
}

impl Balance {
    pub fn parse(self, statement: &mut PartialBrokerStatement, currency: &str) -> EmptyResult {
        if !self.margin.is_zero() || !self.short.is_zero() {
            return Err!("Margin accounts are not supported");
        }

        let cash_assets = validate_decimal(self.cash, DecimalRestrictions::PositiveOrZero)?;
        statement.cash_assets.deposit(Cash::new(currency, cash_assets));

        Ok(())
    }
}