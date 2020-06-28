use num_traits::Zero;
use serde::Deserialize;

use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::util::{DecimalRestrictions, validate_decimal};

use super::common::{DecimalField, Ignore};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Balance {
    #[serde(rename = "AVAILCASH")]
    cash: DecimalField,
    #[serde(rename = "MARGINBALANCE")]
    margin: DecimalField,
    #[serde(rename = "SHORTBALANCE")]
    short: DecimalField,
    #[serde(rename = "BUYPOWER")]
    buy_power: Ignore,
    #[serde(rename = "BALLIST")]
    other: Ignore,
}

impl Balance {
    pub fn parse(self, statement: &mut PartialBrokerStatement, currency: &str) -> EmptyResult {
        if !self.margin.value.is_zero() || !self.short.value.is_zero() {
            return Err!("Margin accounts are not supported");
        }

        let cash_assets = validate_decimal(self.cash.value, DecimalRestrictions::PositiveOrZero)?;
        statement.cash_assets.deposit(Cash::new(currency, cash_assets));

        Ok(())
    }
}