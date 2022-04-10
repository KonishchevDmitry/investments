use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use std::str::FromStr;

use num_traits::{ToPrimitive, Zero};
use separator::Separatable;

use crate::core::{GenericResult, EmptyResult};
use crate::types::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cash {
    pub currency: &'static str,
    pub amount: Decimal,
}

impl Cash {
    pub fn new(currency: &str, amount: Decimal) -> Cash {
        Cash {
            currency: super::name_cache::get(currency),
            amount: amount,
        }
    }

    pub fn zero(currency: &str) -> Cash {
        Cash::new(currency, Decimal::zero())
    }

    pub fn new_from_string(currency: &str, amount: &str) -> GenericResult<Cash> {
        Ok(Cash::new(currency, Decimal::from_str(amount).map_err(|_| format!(
            "Invalid cash amount: {:?}", amount))?))
    }

    pub fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    pub fn is_positive(&self) -> bool {
        !self.amount.is_zero() && self.amount.is_sign_positive()
    }

    pub fn is_negative(&self) -> bool {
        !self.amount.is_zero() && self.amount.is_sign_negative()
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, amount: Cash) -> GenericResult<Cash> {
        self.add_assign(amount)?;
        Ok(self)
    }

    pub fn add_assign(&mut self, amount: Cash) -> EmptyResult {
        self.ensure_same_currency(amount)?;
        self.amount += amount.amount;
        Ok(())
    }

    pub fn sub(self, amount: Cash) -> GenericResult<Cash> {
        self.add(-amount)
    }

    pub fn sub_assign(&mut self, amount: Cash) -> EmptyResult {
        self.add_assign(-amount)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn div(self, amount: Cash) -> GenericResult<Decimal> {
        self.ensure_same_currency(amount)?;
        Ok(self.amount / amount.amount)
    }

    pub fn round(mut self) -> Cash {
        self.amount = super::round(self.amount);
        self
    }

    pub fn round_to(mut self, points: u32) -> Cash {
        self.amount = super::round_to(self.amount, points);
        self
    }

    pub fn normalize(mut self) -> Cash {
        self.amount = self.amount.normalize();
        self
    }

    pub fn format_rounded(&self) -> String {
        let amount = super::round_to(self.amount, 0).to_i64().unwrap().separated_string();
        format_currency(self.currency, &amount)
    }

    fn ensure_same_currency(self, other: Cash) -> EmptyResult {
        if self.currency == other.currency {
            Ok(())
        } else {
            Err!("Currency mismatch: {} and {}", self.currency, other.currency)
        }
    }
}

impl Neg for Cash {
    type Output = Cash;

    fn neg(mut self) -> Cash {
        self.amount = -self.amount;
        self
    }
}

impl Add for Cash {
    type Output = Cash;

    fn add(self, rhs: Cash) -> Cash {
        self.add(rhs).unwrap()
    }
}

impl AddAssign for Cash {
    fn add_assign(&mut self, rhs: Cash) {
        self.add_assign(rhs).unwrap()
    }
}

impl Sub for Cash {
    type Output = Cash;

    fn sub(self, rhs: Cash) -> Cash {
        self.sub(rhs).unwrap()
    }
}

impl SubAssign for Cash {
    fn sub_assign(&mut self, rhs: Cash) {
        self.sub_assign(rhs).unwrap()
    }
}

impl<T> Mul<T> for Cash where T: Into<Decimal> {
    type Output = Cash;

    fn mul(mut self, rhs: T) -> Cash {
        self.amount *= rhs.into();
        self
    }
}

impl Div for Cash {
    type Output = Decimal;

    fn div(self, rhs: Cash) -> Decimal {
        self.div(rhs).unwrap()
    }
}

impl<T> Div<T> for Cash where T: Into<Decimal> {
    type Output = Cash;

    fn div(mut self, rhs: T) -> Cash {
        self.amount /= rhs.into();
        self
    }
}

impl fmt::Display for Cash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut amount = self.amount.normalize();

        if amount.scale() == 1 {
            amount.set_scale(0).unwrap();
            amount = Decimal::new(amount.to_i64().unwrap() * 10, 2)
        }

        write!(f, "{}", format_currency(self.currency, &separated_float!(amount.to_string())))
    }
}

fn format_currency(currency: &str, mut amount: &str) -> String {
    let prefix = match currency {
        "AUD" => Some("AU$"),
        "EUR" => Some("€"),
        "GBP" => Some("£"),
        "USD" => Some("$"),
        _ => None,
    };

    let mut buffer = String::with_capacity(amount.len() + prefix.map(str::len).unwrap_or(1));

    if let Some(prefix) = prefix {
        if amount.starts_with('-') || amount.starts_with('+') {
            buffer.push_str(&amount[..1]);
            amount = &amount[1..];
        }
        buffer.push_str(prefix);
    }

    buffer.push_str(amount);

    if prefix.is_none() {
        match currency {
            "HKD" => buffer.push_str(" HK$"),
            "RUB" => buffer.push('₽'),
            _ => {
                buffer.push(' ');
                buffer.push_str(currency);
            },
        };
    }

    buffer
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(currency, amount, expected,
        case("USD", dec!(12.345), "$12.345"),
        case("USD", dec!(-12.345), "-$12.345"),

        case("RUB", dec!(12.345), "12.345₽"),
        case("RUB", dec!(-12.345), "-12.345₽"),

        case("UNKNOWN", dec!(12.345), "12.345 UNKNOWN"),
        case("UNKNOWN", dec!(-12.345), "-12.345 UNKNOWN"),
    )]
    fn currency_formatting(currency: &str, amount: Decimal, expected: &str) {
        assert_eq!(Cash::new(currency, amount).to_string(), expected);
    }

    #[rstest(input, expected,
        case("12",     "12"),
        case("12.3",   "12.30"),
        case("12.30",  "12.30"),
        case("12.34",  "12.34"),
        case("12.345", "12.345"),
        case("12.001", "12.001"),
    )]
    fn cash_formatting(input: &str, expected: &str) {
        let currency = "CURRENCY";

        for sign in &["", "-"] {
            let input = Cash::new(currency, Decimal::from_str(&format!("{}{}", sign, input)).unwrap());
            let expected = format!("{}{} {}", sign, expected, currency);
            assert_eq!(input.to_string(), expected);
        }
    }
}