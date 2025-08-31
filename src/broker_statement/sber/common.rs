use std::borrow::Cow;

use scraper::{CaseSensitivity, ElementRef};

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

pub fn trim_column_title(title: &str) -> Cow<'_, str> {
    Cow::from(title.trim_end_matches(['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹']))
}

pub fn skip_row(row: ElementRef) -> bool {
    // Row with column numbers
    row.value().has_class("rn", CaseSensitivity::CaseSensitive) ||

    // Various summaries
    row.value().has_class("summary-row", CaseSensitivity::CaseSensitive) ||
    html::select_multiple(row, "td").unwrap().iter().any(|column| column.attr("colspan").unwrap_or("1") != "1")
}