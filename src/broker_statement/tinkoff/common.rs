use crate::core::GenericResult;
use crate::currency::Cash;
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

pub fn parse_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%d.%m.%Y")
}

pub fn parse_decimal(string: &str, restrictions: DecimalRestrictions) -> GenericResult<Decimal> {
    util::parse_decimal(&string.replace(',', "."), restrictions)
}

pub fn parse_cash(currency: &str, value: &str, restrictions: DecimalRestrictions) -> GenericResult<Cash> {
    Ok(Cash::new(currency, parse_decimal(value, restrictions)?))
}