use lazy_static::lazy_static;
use regex::Regex;

use crate::core::GenericResult;
use crate::types::Decimal;

pub fn parse_forex_code(code: &str) -> GenericResult<(&str, &str, Option<Decimal>)> {
    lazy_static! {
        static ref CODE_REGEX: Regex = Regex::new(
            r"^(?P<base>[A-Z]{3})(?P<quote>[A-Z]{3})_(?:TOD|TOM)$").unwrap();
    }

    let (base, quote, lot_size) = match code {
        "USD000000TOD" | "USD000UTSTOM" => ("USD", "RUB", Some(dec!(1000))),
        "EUR_RUB__TOD" | "EUR_RUB__TOM" => ("EUR", "RUB", Some(dec!(1000))),
        _ => {
            let captures = CODE_REGEX.captures(code).ok_or_else(|| format!(
                "Unsupported forex pair code: {:?}", code))?;
            (
                captures.name("base").unwrap().as_str(),
                captures.name("quote").unwrap().as_str(),
                None,
            )
        },
    };

    Ok((base, quote, lot_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forex_code_parsing() {
        assert_eq!(
            parse_forex_code("HKDRUB_TOM").unwrap(),
            ("HKD", "RUB", None),
        );
    }
}