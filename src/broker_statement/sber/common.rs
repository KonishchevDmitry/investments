use crate::core::GenericResult;
use crate::formats::html::{self, Cell};
use crate::time::{self, Date, Time};
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

pub fn parse_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%d.%m.%Y")
}

pub fn parse_date_cell(cell: &Cell) -> GenericResult<Date> {
    parse_date(html::get_string_cell(cell)?)
}

pub fn parse_time_cell(cell: &Cell) -> GenericResult<Time> {
    time::parse_time(html::get_string_cell(cell)?, "%H:%M:%S")
}

pub fn parse_decimal_cell(cell: &Cell) -> GenericResult<Decimal> {
    let value = html::get_string_cell(cell)?.replace(' ', "");
    util::parse_decimal(&value, DecimalRestrictions::No)
}