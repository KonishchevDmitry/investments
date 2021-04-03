use std::borrow::Cow;
use std::ops::Neg;
use std::str::FromStr;

use lazy_static::lazy_static;
use num_traits::Zero;
use regex::Regex;
use rust_decimal::RoundingStrategy;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::types::Decimal;

#[derive(Clone, Copy)]
pub enum DecimalRestrictions {
    No,
    Zero,
    NonZero,
    NegativeOrZero,
    PositiveOrZero,
    StrictlyPositive,
    StrictlyNegative,
}

pub fn parse_decimal(string: &str, restrictions: DecimalRestrictions) -> GenericResult<Decimal> {
    let value = Decimal::from_str(string).map_err(|_| "Invalid decimal value")?;
    validate_decimal(value, restrictions)
}

pub fn validate_decimal(value: Decimal, restrictions: DecimalRestrictions) -> GenericResult<Decimal> {
    if !match restrictions {
        DecimalRestrictions::No => true,
        DecimalRestrictions::Zero => value.is_zero(),
        DecimalRestrictions::NonZero => !value.is_zero(),
        DecimalRestrictions::NegativeOrZero => value.is_sign_negative() || value.is_zero(),
        DecimalRestrictions::PositiveOrZero => value.is_sign_positive() || value.is_zero(),
        DecimalRestrictions::StrictlyPositive => value.is_sign_positive() && !value.is_zero(),
        DecimalRestrictions::StrictlyNegative => value.is_sign_negative() && !value.is_zero(),
    } {
        return Err!("The value doesn't comply to the specified restrictions");
    }

    Ok(value)
}

pub fn validate_named_decimal(name: &str, value: Decimal, restrictions: DecimalRestrictions) -> GenericResult<Decimal> {
    Ok(validate_decimal(value, restrictions).map_err(|e| format!(
        "Invalid {} ({}): {}", name, value, e))?)
}

pub fn validate_named_cash(name: &str, currency: &str, value: Decimal, restrictions: DecimalRestrictions) -> GenericResult<Cash> {
    Ok(Cash::new(currency, validate_named_decimal(name, value, restrictions)?))
}

pub fn decimal_precision(value: Decimal) -> u32 {
    value.fract().scale()
}

pub fn round(value: Decimal, points: u32) -> Decimal {
    round_with(value, points, RoundingMethod::Round)
}

#[derive(Clone, Copy, Debug)]
pub enum RoundingMethod {
    Round,
    Truncate,
}

pub fn round_with(value: Decimal, points: u32, method: RoundingMethod) -> Decimal {
    let mut round_value = match method {
        RoundingMethod::Round => value.round_dp_with_strategy(points, RoundingStrategy::RoundHalfUp),
        RoundingMethod::Truncate => {
            let mut value = value;
            let scale = value.scale();

            if scale > points {
                value.set_scale(scale - points).unwrap();
                value = value.trunc();
                value.set_scale(points).unwrap();
            }

            value
        },
    };

    if round_value.is_zero() && round_value.is_sign_negative() {
        round_value = round_value.neg();
    }

    round_value.normalize()
}

pub fn fold_spaces(string: &str) -> Cow<str> {
    lazy_static! {
        static ref SPACES_REGEX: Regex = Regex::new(r"\s{2,}").unwrap();
    }
    SPACES_REGEX.replace_all(string, " ")
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(num, scale, precision,
        case(321, 0, 0),
        case(321, 1, 1),
        case(321, 2, 2),
        case(321, 3, 3),
        case(321, 4, 4),

        case(3210, 0, 0),
        case(3210, 1, 1),
        case(3210, 2, 2),
        case(3210, 3, 3),
        case(3210, 4, 4),
        case(3210, 5, 5),
    )]
    fn decimal_precision(num: i64, scale: u32, precision: u32) {
        let value = Decimal::new(num, scale);
        assert_eq!(super::decimal_precision(value), precision)
    }

    #[rstest(value, expected,
        case(dec!(-1.5), dec!(-2)),
        case(dec!(-1.4), dec!(-1)),
        case(dec!(-1),   dec!(-1)),
        case(dec!(-0.5), dec!(-1)),
        case(dec!(-0.4), dec!(0)),
        case(dec!( 0), dec!(0)),
        case(dec!(-0), dec!(0)),
        case(dec!(0.4), dec!(0)),
        case(dec!(0.5), dec!(1)),
        case(dec!(1),   dec!(1)),
        case(dec!(1.4), dec!(1)),
        case(dec!(1.5), dec!(2)),
    )]
    fn rounding(value: Decimal, expected: Decimal) {
        assert_eq!(round(value, 0), expected);
    }

    #[rstest(value, expected,
        case(dec!(-1.6), dec!(-1)),
        case(dec!(-1.4), dec!(-1)),
        case(dec!(-1),   dec!(-1)),
        case(dec!(-0.6), dec!(0)),
        case(dec!(-0.4), dec!(0)),
        case(dec!( 0), dec!(0)),
        case(dec!(-0), dec!(0)),
        case(dec!(0.4), dec!(0)),
        case(dec!(0.6), dec!(0)),
        case(dec!(1),   dec!(1)),
        case(dec!(1.4), dec!(1)),
        case(dec!(1.6), dec!(1)),
    )]
    fn truncate_rounding(value: Decimal, expected: Decimal) {
        assert_eq!(round_with(value, 0, RoundingMethod::Truncate), expected);
    }
}