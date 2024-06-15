#[cfg(test)] use std::str::FromStr;

use crate::currency;
use crate::localities::Jurisdiction;
use crate::types::Decimal;

pub trait TaxRate {
    fn tax(&mut self, income: Decimal) -> Decimal;
}

pub struct FixedTaxRate {
    rate: Decimal,
    precision: u32,
}

impl FixedTaxRate {
    pub fn new(rate: Decimal, precision: u32) -> FixedTaxRate {
        FixedTaxRate {rate, precision}
    }
}

impl TaxRate for FixedTaxRate {
    fn tax(&mut self, income: Decimal) -> Decimal {
        let income = currency::round(income);
        if income.is_sign_negative() || income.is_zero() {
            return dec!(0);
        }
        round_tax(income * self.rate, self.precision)
    }
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
fn round_tax(tax: Decimal, precision: u32) -> Decimal {
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
        let result = round_tax(tax, Jurisdiction::Russia.tax_precision());
        assert_eq!(result, expected.parse().unwrap());
    }

    #[rstest(income, expected,
        case("100",    "13"),
        case("100.00", "13"),
        case("103.80", "13"),
        case("103.81", "14"),
        case("103.85", "14"),
    )]
    fn fixed_tax_rate(income: &str, expected: &str) {
        let mut calc = FixedTaxRate::new(dec!(0.13), Jurisdiction::Russia.tax_precision());
        let tax = calc.tax(income.parse().unwrap());
        assert_eq!(tax, expected.parse().unwrap());
    }
}