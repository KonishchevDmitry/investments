mod calculator;
pub mod long_term_ownership;
mod net_calculator;
mod payment_day;
mod rates;
mod remapping;

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::brokers::Broker;
use crate::core::EmptyResult;
use crate::currency;
use crate::localities::Jurisdiction;
use crate::types::Decimal;

pub use self::calculator::{TaxCalculator, Tax};
pub use self::long_term_ownership::{
    LtoDeductibleProfit, LtoDeductionCalculator, LtoDeduction,
    NetLtoDeduction, NetLtoDeductionCalculator};
pub use self::net_calculator::{NetTax, NetTaxCalculator};
pub use self::payment_day::{TaxPaymentDay, TaxPaymentDaySpec};
pub use self::rates::{TaxRate, FixedTaxRate, ProgressiveTaxRate};
pub use self::remapping::TaxRemapping;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaxConfig {
    pub income: BTreeMap<i32, Decimal>
}

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

// When we work with taxes in Russia, the following rounding rules are applied:
// 1. Result of all calculations must be with kopecks precision
// 2. If we have income in foreign currency then:
//    2.1. Round it to cents
//    2.2. Convert to rubles using precise currency rate (65.4244 for example)
//    2.3. Round to kopecks
// 3. Taxes are calculated with rouble precision. But we should use double rounding here:
//    calculate them with kopecks precision first and then round to roubles.
//
// Декларация program allows to enter income only with kopecks precision - not bigger.
// It calculates tax for $10.64 income with 65.4244 currency rate as following:
// 1. income = round(10.64 * 65.4244, 2) = 696.12 (696.115616 without rounding)
// 2. tax = round(round(696.12 * 0.13, 2), 0) = 91 (90.4956 without rounding)
pub fn round_tax(tax: Decimal, precision: u32) -> Decimal {
    currency::round_to(currency::round(tax), precision)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(tax, expected,
        case("13",      "13"),
        case("13.0000", "13"),
        case("13.1111", "13"),
        case("13.4949", "13"),
        case("13.4950", "14"),
        case("13.9999", "14"),
    )]
    fn tax_rounding(tax: &str, expected: &str) {
        let tax = tax.parse().unwrap();
        let result = round_tax(tax, Jurisdiction::Russia.traits().tax_precision);
        assert_eq!(result, expected.parse().unwrap());
    }
}