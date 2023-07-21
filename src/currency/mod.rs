#[cfg(test)] use std::str::FromStr;

use lazy_static::lazy_static;
use regex::Regex;
use validator::ValidationError;

use crate::time::Date;
use crate::types::Decimal;
use crate::util;

mod cash;
mod cbr;
mod multi;
mod name_cache;
mod rate_cache;

pub mod converter;

pub use self::cash::{Cash, CashAssets};
pub use self::multi::MultiCurrencyCashAccount;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct CurrencyRate {
    date: Date,
    price: Decimal,
}

pub fn round(amount: Decimal) -> Decimal {
    util::round(amount, 2)
}

pub fn round_to(amount: Decimal, points: u32) -> Decimal {
    util::round(amount, points)
}

pub fn validate_currency(currency: &str) -> Result<(), ValidationError> {
    lazy_static! {
        static ref CURRENCY_REGEX: Regex = Regex::new(r"^[A-Z]{3}$").unwrap();
    }
    if !CURRENCY_REGEX.is_match(currency) {
        return Err(ValidationError::new("Invalid currency"));
    }
    Ok(())
}

pub fn validate_currency_list<C, I>(currencies: C) -> Result<(), ValidationError>
    where
        C: IntoIterator<Item = I>,
        I: AsRef<str>,
{
    for currency in currencies.into_iter() {
        validate_currency(currency.as_ref())?;
    }
    Ok(())
}

fn format_currency(currency: &str, mut amount: &str) -> String {
    let prefix = match currency {
        "AUD" => Some("AU$"),
        "CNY" => Some("¥"),
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

    #[rstest(input, expected,
        case("1",     "1"),
        case("1.0",   "1"),
        case("1.1",   "1.1"),
        case("1.00",  "1"),
        case("1.01",  "1.01"),
        case("1.11",  "1.11"),
        case("1.004", "1"),
        case("1.005", "1.01"),
        case("1.111", "1.11"),
        case("1.114", "1.11"),
        case("1.124", "1.12"),
        case("1.115", "1.12"),
        case("1.125", "1.13"),
    )]
    fn rounding(input: &str, expected: &str) {
        let from = Decimal::from_str(input).unwrap();
        let to = Decimal::from_str(expected).unwrap();

        let rounded = round(from);
        assert_eq!(rounded, to);

        assert_eq!(&from.to_string(), input);
        assert_eq!(&rounded.to_string(), expected);
    }

    #[rstest(currency, amount, expected,
        case("USD", dec!(12.345), "$12.345"),
        case("USD", dec!(-12.345), "-$12.345"),

        case("RUB", dec!(12.345), "12.345₽"),
        case("RUB", dec!(-12.345), "-12.345₽"),

        case("UNKNOWN", dec!(12.345), "12.345 UNKNOWN"),
        case("UNKNOWN", dec!(-12.345), "-12.345 UNKNOWN"),
    )]
    fn formatting(currency: &str, amount: Decimal, expected: &str) {
        assert_eq!(Cash::new(currency, amount).to_string(), expected);
    }
}