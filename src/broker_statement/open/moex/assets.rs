use std::collections::HashMap;

use serde::Deserialize;

use crate::broker_statement::open::common::{parse_quantity, get_symbol};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::types::Decimal;

#[derive(Deserialize)]
pub struct AccountSummary {
    #[serde(rename = "item")]
    items: Vec<AccountSummaryItem>,
}

#[derive(Deserialize)]
struct AccountSummaryItem {
    #[serde(rename = "row_name")]
    name: String,

    #[serde(rename = "value")]
    amount: Decimal,
}

impl AccountSummary {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        let mut has_starting_assets = None;

        for item in &self.items {
            if item.name == "Входящий остаток (факт)" {
                let has_assets = !item.amount.is_zero();
                has_starting_assets.replace(has_starting_assets.unwrap_or_default() | has_assets);
            }
        }

        let has_starting_assets = has_starting_assets.ok_or(
            "Unable to find starting cash assets information")?;

        statement.set_has_starting_assets(has_starting_assets)
    }
}

#[derive(Deserialize)]
pub struct Assets {
    #[serde(rename = "item")]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    #[serde(rename = "asset_type")]
    type_: String,

    #[serde(rename = "asset_name")]
    name: String,

    #[serde(rename = "asset_code")]
    code: String,

    #[serde(rename = "opening_position_plan")]
    start_amount: Decimal,

    #[serde(rename = "closing_position_plan")]
    end_amount: Decimal,
}

impl Assets {
    pub fn parse(&self, statement: &mut PartialBrokerStatement, securities: &HashMap<String, String>) -> EmptyResult {
        let mut has_starting_assets = false;

        for asset in &self.assets {
            has_starting_assets |= !asset.start_amount.is_zero();

            match asset.type_.as_str() {
                "Акции" | "ПАИ" | "GDR" => {
                    let symbol = get_symbol(securities, &asset.name)?;
                    let amount = parse_quantity(asset.end_amount, true)?;
                    if amount != 0 {
                        statement.add_open_position(symbol, amount.into())?
                    }
                },
                "Денежные средства" => {
                    statement.assets.cash.as_mut().unwrap().deposit(
                        Cash::new(&asset.code, asset.end_amount));
                },
                _ => return Err!("Unsupported asset type: {:?}", asset.type_),
            };
        }

        if has_starting_assets {
            statement.has_starting_assets = Some(true);
        }

        Ok(())
    }
}

#[derive(Deserialize)]
pub struct Securities {
    #[serde(rename = "item")]
    securities: Vec<Security>,
}

#[derive(Deserialize)]
struct Security {
    // There is also `issuer_name` field which contains much more human readable names, but it's
    // actually not security name - it's dividend issuer name. For example for GDR it will contain
    // BNY Mellon / Citibank N.A. instead of actual stock name.
    #[serde(rename = "security_name")]
    name: String,
    isin: String,
    #[serde(rename = "ticker")]
    symbol: String,
}

impl Securities {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> GenericResult<HashMap<String, String>> {
        let mut securities = HashMap::new();

        for security in &self.securities {
            if securities.insert(security.name.clone(), security.symbol.clone()).is_some() {
                return Err!("Duplicated security name: {:?}", security.name);
            }

            let instrument = statement.instrument_info.add(&security.symbol)?;
            instrument.set_name(parse_security_name(&security.name));
            instrument.add_isin(&security.isin)?;
        }

        Ok(securities)
    }
}

pub fn parse_security_name(name: &str) -> &str {
    name.trim_end().trim_end_matches('_')
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(name, expected,
        case("AGRO-гдр  ", "AGRO-гдр"),
        case("FXUK ETF_", "FXUK ETF"),
    )]
    fn security_name_parsing(name: &str, expected: &str) {
        assert_eq!(parse_security_name(name), expected);
    }
}