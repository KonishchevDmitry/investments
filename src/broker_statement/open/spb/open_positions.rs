use serde::Deserialize;

use crate::broker_statement::open::common::{InstrumentType, parse_quantity};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::GenericResult;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::common::parse_security_code;

#[derive(Deserialize)]
pub struct OpenPositions {
    #[serde(rename = "item")]
    open_positions: Vec<OpenPosition>,
}

#[derive(Deserialize)]
struct OpenPosition {
    #[serde(rename = "sharecode")]
    code: String,
    #[serde(rename = "isin")]
    isin: String,
    #[serde(rename = "categoryname")]
    category: String,
    #[serde(rename = "issuername")]
    issuer: String,
    #[serde(rename = "onaccountbegin")]
    start_quantity: Decimal,
    #[serde(rename = "plannedbalance")]
    end_quantity: Decimal,
}

impl OpenPositions {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> GenericResult<bool> {
        let mut has_starting_assets = false;

        for security in &self.open_positions {
            let start_quantity = util::validate_named_decimal(
                "open position quantity", parse_quantity(security.start_quantity),
                DecimalRestrictions::PositiveOrZero)?;

            let end_quantity = util::validate_named_decimal(
                "open position quantity", parse_quantity(security.end_quantity),
                DecimalRestrictions::PositiveOrZero)?;

            has_starting_assets |= !start_quantity.is_zero();

            let instrument_type = InstrumentType::parse(&security.category)?;
            let symbol = parse_security_code(&security.code)?;

            if !end_quantity.is_zero() {
                statement.add_open_position(symbol, end_quantity)?
            }

            let instrument = statement.instrument_info.add(symbol)?;
            instrument.add_isin(&security.isin)?;

            match instrument_type {
                InstrumentType::Stock => {
                    instrument.set_name(&security.issuer);
                }
                InstrumentType::DepositaryReceipt => {},
            };
        }

        Ok(has_starting_assets)
    }
}