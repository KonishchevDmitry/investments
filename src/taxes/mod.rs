mod long_term_ownership;
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
    LongTermOwnership,
    TaxFree,
}

impl<'de> Deserialize<'de> for TaxExemption {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            // FIXME(konishchev): Support
            "tax-free" => TaxExemption::TaxFree,
            _ => return Err(D::Error::unknown_variant(&value, &["tax-free"])),
        })
    }
}