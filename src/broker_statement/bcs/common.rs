use crate::core::GenericResult;
use crate::types::Date;
use crate::util;

pub fn parse_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%d.%m.%Y")
}