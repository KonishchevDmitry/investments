use num_traits::cast::ToPrimitive;
use serde::Deserialize;

use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::EmptyResult;
use crate::util::{self, DecimalRestrictions};

use super::common::{Ignore, validate_sub_account};
use super::security_info::{SecurityInfo, SecurityId, SecurityType};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenPositions {
    #[serde(rename = "POSSTOCK")]
    stocks: Vec<OpenStockPosition>,
}

impl OpenPositions {
    pub fn parse(self, statement: &mut PartialBrokerStatement, securities: &SecurityInfo) -> EmptyResult {
        for stock in self.stocks {
            stock.open_position.parse(statement, securities)?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenStockPosition {
    #[serde(rename = "INVPOS")]
    open_position: OpenPosition,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenPosition {
    #[serde(rename = "SECID")]
    security_id: SecurityId,
    #[serde(rename = "HELDINACCT")]
    sub_account: String,
    #[serde(rename = "POSTYPE")]
    _type: String,
    #[serde(rename = "UNITS")]
    units: String,
    #[serde(rename = "UNITPRICE")]
    _price: Ignore,
    #[serde(rename = "MKTVAL")]
    _volume: Ignore,
    #[serde(rename = "DTPRICEASOF")]
    _price_date: Ignore,
    #[serde(rename = "MEMO")]
    _memo: Ignore,
}

impl OpenPosition {
    fn parse(self, statement: &mut PartialBrokerStatement, securities: &SecurityInfo) -> EmptyResult {
        if self._type != "LONG" {
            return Err!("Unsupported {} open position type: {:?}", self.security_id, self._type);
        }
        validate_sub_account(&self.sub_account)?;

        let symbol = match securities.get(&self.security_id)? {
            SecurityType::Stock(symbol) => symbol,
            _ => return Err!("Got {} open position with an unexpected security type", self.security_id),
        };

        let quantity = util::parse_decimal(&self.units, DecimalRestrictions::StrictlyPositive)
            .ok().and_then(|quantity| {
                if quantity.trunc() == quantity {
                    quantity.abs().to_u32()
                } else {
                    None
                }
            }).ok_or_else(|| format!("Invalid {} open positions quantity: {:?}", symbol, self.units))?;

        statement.add_open_position(symbol, quantity.into())
    }
}