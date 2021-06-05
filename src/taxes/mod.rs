mod net_calculator;
mod payment_day;
mod remapping;

use serde::Deserialize;
use serde::de::{Deserializer, Error};

pub use self::net_calculator::NetTaxCalculator;
pub use self::payment_day::{TaxPaymentDay, TaxPaymentDaySpec};
pub use self::remapping::TaxRemapping;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum IncomeType {
    Trading,
    Dividends,
    Interest,
}

#[derive(Clone, Copy, Debug)]
pub enum TaxExemption {
    TaxFree,
}

impl TaxExemption {
    pub fn is_applicable(&self) -> (bool, bool) {
        match self {
            TaxExemption::TaxFree => (true, true),
        }
    }
}

impl<'de> Deserialize<'de> for TaxExemption {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "tax-free" => TaxExemption::TaxFree,
            _ => return Err(D::Error::unknown_variant(&value, &["tax-free"])),
        })
    }
}