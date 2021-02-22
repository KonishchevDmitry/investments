use crate::core::GenericResult;

pub fn parse_forex_code(code: &str) -> GenericResult<(&str, &str)> {
    let (base, quote) = match code {
        "USD000000TOD" | "USD000UTSTOM" => ("USD", "RUB"),
        "EUR_RUB__TOD" | "EUR_RUB__TOM" => ("EUR", "RUB"),
        _ => return Err!("Unsupported forex pair code: {:?}", code),
    };
    Ok((base, quote))
}