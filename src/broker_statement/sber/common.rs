use crate::core::GenericResult;
use crate::formats::xls::{self, Cell, CellType};
use crate::time;
use crate::types::{Date, Time, Decimal};
use crate::util::{self, DecimalRestrictions};

// XXX(konishchev): Rewrite
pub fn parse_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%d.%m.%Y")
}

pub fn parse_date_cell(cell: &Cell) -> GenericResult<Date> {
    parse_date(xls::get_string_cell(cell)?)
}

pub fn parse_time(time: &str) -> GenericResult<Time> {
    time::parse_time(time, "%H:%M:%S")
}

pub fn parse_time_cell(cell: &Cell) -> GenericResult<Time> {
    parse_time(xls::get_string_cell(cell)?)
}

pub fn parse_decimal_cell(cell: &Cell) -> GenericResult<Decimal> {
    match cell {
        Cell::String(value) => {
            let value = value.replace(' ', "");
            util::parse_decimal(&value, DecimalRestrictions::No)
        },
        _ => Decimal::parse(cell, true),
    }
}