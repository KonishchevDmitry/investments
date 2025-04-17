use std::collections::HashSet;
use std::str::FromStr;

use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::types::Decimal;
use crate::util;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetAllocationConfig {
    pub name: String,
    pub symbol: Option<String>,

    #[serde(deserialize_with = "deserialize_weight")]
    pub weight: Decimal,
    pub restrict_buying: Option<bool>,
    pub restrict_selling: Option<bool>,

    pub assets: Option<Vec<AssetAllocationConfig>>,
}

impl AssetAllocationConfig {
    pub fn get_stock_symbols(&self, symbols: &mut HashSet<String>) {
        if let Some(ref symbol) = self.symbol {
            symbols.insert(symbol.to_owned());
        }

        if let Some(ref assets) = self.assets {
            for asset in assets {
                asset.get_stock_symbols(symbols);
            }
        }
    }
}

fn deserialize_weight<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
    where D: Deserializer<'de>
{
    let weight: String = Deserialize::deserialize(deserializer)?;

    let weight = Some(weight.as_str())
        .and_then(|weight| weight.strip_suffix('%'))
        .and_then(|weight| Decimal::from_str(weight).ok())
        .and_then(|weight| {
            if weight.is_sign_positive() && util::decimal_precision(weight) <= 2 && weight <= dec!(100) {
                Some(weight.normalize())
            } else {
                None
            }
        }).ok_or_else(|| D::Error::custom(format!("Invalid weight: {}", weight)))?;

    Ok(weight / dec!(100))
}