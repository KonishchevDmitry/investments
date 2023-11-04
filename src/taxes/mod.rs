pub mod long_term_ownership;
mod net_calculator;
mod payment_day;
mod remapping;

use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::brokers::Broker;
use crate::core::EmptyResult;
use crate::localities::Jurisdiction;

pub use self::long_term_ownership::{
    LtoDeductibleProfit, LtoDeductionCalculator, LtoDeduction,
    NetLtoDeduction, NetLtoDeductionCalculator};
pub use self::net_calculator::{NetTax, NetTaxCalculator};
pub use self::payment_day::{TaxPaymentDay, TaxPaymentDaySpec};
pub use self::remapping::TaxRemapping;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum IncomeType {
    Trading,
    Dividends,
    Interest,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TaxExemption {
    LongTermOwnership,
    TaxFree,
}

impl<'de> Deserialize<'de> for TaxExemption {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "long-term-ownership" => TaxExemption::LongTermOwnership,
            "tax-free" => TaxExemption::TaxFree,
            _ => return Err(D::Error::unknown_variant(&value, &["long-term-ownership", "tax-free"])),
        })
    }
}

pub fn validate_tax_exemptions(broker: Broker, exemptions: &[TaxExemption]) -> EmptyResult {
    if exemptions.is_empty() {
        return Ok(());
    }

    if exemptions.len() > 1 {
        return Err!("Only one tax exemption can be specified per portfolio");
    }

    if broker.jurisdiction() != Jurisdiction::Russia {
        return Err!("Tax exemptions are only supported for brokers with Russia jurisdiction");
    }

    Ok(())
}