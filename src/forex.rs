use crate::core::GenericResult;
use crate::types::Decimal;

pub fn parse_forex_code(code: &str) -> GenericResult<(&str, &str, Decimal)> {
    let (base, quote) = match code {
        "USD000000TOD" | "USD000UTSTOM" => ("USD", "RUB"),
        "EUR_RUB__TOD" | "EUR_RUB__TOM" => ("EUR", "RUB"),
        _ => return Err!("Unsupported forex pair code: {:?}", code),
    };
    let lot_size = dec!(1000);
    Ok((base, quote, lot_size))
}