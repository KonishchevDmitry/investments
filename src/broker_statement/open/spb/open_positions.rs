use serde::Deserialize;

use crate::broker_statement::open::common::{InstrumentType, parse_quantity};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::GenericResult;
use crate::types::Decimal;

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
            has_starting_assets |= parse_quantity(security.start_quantity, true)? != 0;

            let instrument_type = InstrumentType::parse(&security.category)?;
            let symbol = parse_security_code(&security.code)?;

            let quantity = parse_quantity(security.end_quantity, true)?;
            if quantity != 0 {
                statement.add_open_position(symbol, quantity.into())?
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

fn parse_security_code(code: &str) -> GenericResult<&str> {
    match code.strip_suffix("_SPB") {
        Some(symbol) => Ok(symbol),
        None => Err!("Got a security code in an unexpected format: {:?}", code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_code_parsing() {
        assert_eq!(parse_security_code("KO_SPB").unwrap(), "KO");
    }
}