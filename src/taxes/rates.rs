use std::collections::BTreeMap;
use std::ops::Bound;
use std::rc::Rc;

use dyn_clone::DynClone;
#[cfg(test)] use itertools::Itertools;

use crate::currency;
#[cfg(test)] use crate::localities::Jurisdiction;
use crate::taxes::{self, IncomeType};
use crate::types::Decimal;


pub trait TaxRate: DynClone {
    fn tax(&mut self, income_type: IncomeType, income: Decimal) -> Decimal;
}

dyn_clone::clone_trait_object!(TaxRate);

#[derive(Clone)]
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
    fn tax(&mut self, _income_type: IncomeType, income: Decimal) -> Decimal {
        if income.is_sign_negative() {
            return dec!(0);
        }
        taxes::round_tax(currency::round(income) * self.rate, self.precision)
    }
}

#[derive(Clone)]
pub struct ProgressiveTaxRate {
    rates: Rc<BTreeMap<Decimal, Decimal>>,
    precision: u32,
    tax_base: Decimal,
}

impl ProgressiveTaxRate {
    pub fn new(income: Decimal, rates: Rc<BTreeMap<Decimal, Decimal>>, precision: u32) -> ProgressiveTaxRate {
        ProgressiveTaxRate {
            rates, precision,
            tax_base: std::cmp::max(dec!(0), income),
        }
    }
}

impl TaxRate for ProgressiveTaxRate {
    fn tax(&mut self, _income_type: IncomeType, mut income: Decimal) -> Decimal {
        income = currency::round(income);

        let mut tax = dec!(0);

        while !income.is_zero() && income.is_sign_positive() {
            let (_, &current_rate) = self.rates.range((Bound::Unbounded, Bound::Included(self.tax_base))).last().unwrap();

            let current_income = match self.rates.range((Bound::Excluded(self.tax_base), Bound::Unbounded)).next() {
                Some((&next_rate_tax_base, _)) => std::cmp::min(next_rate_tax_base - self.tax_base, income),
                None => income,
            };

            income -= current_income;
            self.tax_base += current_income;
            tax += taxes::round_tax(current_income * current_rate, self.precision);
        }

        tax
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(income, expected,
        case("0", "0"),
        case("1", "0"),
        case("10", "1"),
        case("100", "13"),

        case("100.00", "13"),
        case("103.80", "13"),
        case("103.81", "14"),
        case("103.85", "14"),

        case("10_000_000", "1_300_000"),
        case("-10_000_000", "0"),
    )]
    fn fixed_tax_rate(income: &str, expected: &str) {
        let mut calc = FixedTaxRate::new(dec!(0.13), Jurisdiction::Russia.traits().tax_precision);
        let tax = calc.tax(IncomeType::Trading, income.parse().unwrap());
        assert_eq!(tax, expected.parse().unwrap());
    }

    #[rstest(initial_income, incomes, expected,
        case(         "0", &["0"],                              &["0"]),
        case(         "0", &["1"],                              &["0"]),
        case(         "0", &["10"],                             &["1"]),
        case(         "0", &["100"],                            &["13"]),
        case(         "0", &["5_000_000"],                      &["650_000"]),
        case(         "0", &["4_999_900", "100"],               &["649_987", "13"]),
        case(         "0", &["5_000_000", "100"],               &["650_000", "15"]),
        case(         "0", &["5_000_100"],                      &["650_015"]),
        case(         "0", &["2_500_000", "2_500_100"],         &["325_000", "325_015"]),
        case( "1_000_000", &["1_500_000", "2_500_100"],         &["195_000", "325_015"]),
        case("10_000_000", &["1_000_000"],                      &["150_000"]),
        case("-9_000_000", &["-9_000_000", "5_000_000", "100"], &["0", "650_000", "15"]),
    )]
    fn progressive_tax_rate(initial_income: &str, incomes: &[&str], expected: &[&str]) {
        let initial_income = initial_income.parse().unwrap();

        let mut calc = ProgressiveTaxRate::new(initial_income, Rc::new(btreemap!{
                    dec!(0) => dec!(0.13),
            dec!(5_000_000) => dec!(0.15),
        }), Jurisdiction::Russia.traits().tax_precision);

        for (income, expected) in incomes.iter().zip_eq(expected) {
            let tax = calc.tax(IncomeType::Trading, income.parse().unwrap());
            assert_eq!(tax, expected.parse().unwrap());
        }
    }
}