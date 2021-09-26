use std::collections::HashMap;

use serde::Deserialize;

use crate::broker_statement::open::common::{InstrumentType, parse_quantity};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::GenericResult;
use crate::currency::Cash;
use crate::types::Decimal;

use super::common::get_symbol;

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
    pub fn parse(&self, statement: &mut PartialBrokerStatement, securities: &HashMap<String, String>) -> GenericResult<bool> {
        let mut has_starting_assets = false;

        for asset in &self.assets {
            has_starting_assets |= !asset.start_amount.is_zero();

            match InstrumentType::parse(&asset.type_) {
                Ok(InstrumentType::Stock | InstrumentType::DepositaryReceipt) => {
                    let symbol = get_symbol(securities, &asset.name)?;
                    let quantity = parse_quantity(asset.end_amount, true)?;
                    if quantity != 0 {
                        statement.add_open_position(symbol, quantity.into())?
                    }
                },
                Err(_) if asset.type_ == "Денежные средства" => {
                    statement.assets.cash.as_mut().unwrap().deposit(
                        Cash::new(&asset.code, asset.end_amount));
                },
                _ => return Err!("Unsupported asset type: {:?}", asset.type_),
            }
        }

        Ok(has_starting_assets)
    }
}

#[derive(Deserialize)]
pub struct Securities {
    #[serde(rename = "item")]
    securities: Vec<Security>,
}

#[derive(Deserialize)]
struct Security {
    #[serde(rename = "security_name")]
    name: String,
    #[serde(rename = "issuer_name")]
    issuer: String,
    isin: String,
    #[serde(rename = "security_type")]
    type_: String,
    #[serde(rename = "ticker")]
    symbol: String,
}

impl Securities {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> GenericResult<HashMap<String, String>> {
        let mut securities = HashMap::new();

        for security in &self.securities {
            let name = match InstrumentType::parse(&security.type_)? {
                InstrumentType::Stock => parse_issuer_name(&security.issuer),
                InstrumentType::DepositaryReceipt => parse_security_name(&security.name),
            };

            if securities.insert(security.name.clone(), security.symbol.clone()).is_some() {
                return Err!("Duplicated security name: {:?}", security.name);
            }

            let instrument = statement.instrument_info.add(&security.symbol)?;
            instrument.set_name(name);
            instrument.add_isin(&security.isin)?;
        }

        Ok(securities)
    }
}

fn parse_security_name(name: &str) -> &str {
    name.trim_end().trim_end_matches('_')
}

fn parse_issuer_name(mut issuer: &str) -> &str {
    if let Some(index) = issuer.find("п/у") {
        if index != 0 {
            issuer = &issuer[..index];
        }
    }

    if let Some(index) = issuer.find('(') {
        if index != 0 {
            issuer = &issuer[..index];
        }
    }

    issuer.trim()
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

    #[test]
    fn issuer_name_parsing() {
        assert_eq!(parse_issuer_name(
            "FinEx MSCI China UCITS ETF (USD Share Class) п/у FinEx Investment Management LLP"),
            "FinEx MSCI China UCITS ETF");
    }
}