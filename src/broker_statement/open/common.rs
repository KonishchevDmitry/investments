use serde::{de::Error, Deserialize, Deserializer};

use crate::core::GenericResult;
use crate::time::{self, Date};
use crate::types::Decimal;

pub enum InstrumentType {
    Stock,
    DepositaryReceipt,
}

impl InstrumentType {
    pub fn parse(name: &str) -> GenericResult<InstrumentType> {
        Ok(match name {
            "Акции" | "АО" | "ПАИ" => InstrumentType::Stock,
            "ADR" | "GDR" => InstrumentType::DepositaryReceipt,
            _ => return Err!("Unsupported instrument type: {:?}", name),
        })
    }
}

fn parse_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%Y-%m-%dT00:00:00")
}

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<Date, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    parse_date(&value).map_err(D::Error::custom)
}

pub fn parse_quantity(quantity: Decimal) -> Decimal {
    quantity.normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_parsing() {
        assert_eq!(
            parse_date("2017-12-31T00:00:00").unwrap(),
            date!(2017, 12, 31),
        );
    }
}