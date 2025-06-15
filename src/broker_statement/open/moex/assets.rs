use std::collections::HashMap;

use serde::Deserialize;

use crate::broker_statement::open::common::{InstrumentType, parse_quantity};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::common::get_symbol;

#[derive(Deserialize)]
pub struct AccountSummary {
    #[serde(rename = "item")]
    items: Vec<AccountSummaryItem>,
}

#[derive(Deserialize)]
struct AccountSummaryItem {
    #[serde(rename = "@row_name")]
    name: String,

    #[serde(rename = "@value")]
    amount: Decimal,
}

impl AccountSummary {
    pub fn parse(&self) -> GenericResult<bool> {
        let mut has_starting_assets = None;

        for item in &self.items {
            if item.name == "Входящий остаток (факт)" {
                let has_assets = !item.amount.is_zero();
                has_starting_assets.replace(has_starting_assets.unwrap_or_default() | has_assets);
            }
        }

        Ok(has_starting_assets.ok_or("Unable to find starting cash assets information")?)
    }
}

#[derive(Deserialize)]
pub struct Assets {
    #[serde(rename = "item")]
    assets: Vec<Asset>,
}

impl Assets {
    pub fn parse(&self, statement: &mut PartialBrokerStatement, securities: &HashMap<String, String>) -> GenericResult<bool> {
        let mut has_starting_assets = false;

        for asset in &self.assets {
            has_starting_assets |= !asset.start_amount.is_zero();
            asset.parse(statement, securities)?;
        }

        Ok(has_starting_assets)
    }
}

#[derive(Deserialize)]
struct Asset {
    #[serde(rename = "@asset_type")]
    type_: String,

    #[serde(rename = "@asset_name")]
    name: String,

    #[serde(rename = "@asset_code")]
    code: String,

    #[serde(rename = "@opening_position_plan")]
    start_amount: Decimal,

    #[serde(rename = "@closing_position_plan")]
    end_amount: Decimal,
}

impl Asset {
    fn parse(&self, statement: &mut PartialBrokerStatement, securities: &HashMap<String, String>) -> EmptyResult {
        match InstrumentType::parse(&self.type_) {
            Ok(InstrumentType::Stock | InstrumentType::DepositaryReceipt) => {
                let symbol = get_symbol(securities, &self.name)?;

                let quantity = util::validate_named_decimal(
                    "open position quantity", parse_quantity(self.end_amount),
                    DecimalRestrictions::PositiveOrZero)?;

                if !quantity.is_zero() {
                    statement.add_open_position(symbol, quantity)?
                }
            },
            Err(_) if self.type_ == "Денежные средства" => {
                statement.assets.cash.as_mut().unwrap().deposit(
                    Cash::new(&self.code, self.end_amount));
            },
            _ => return Err!("Unsupported asset type: {:?}", self.type_),
        }

        Ok(())
    }
}