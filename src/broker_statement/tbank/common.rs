use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use isin::ISIN;

use crate::core::{EmptyResult, GenericResult};
use crate::exchanges::Exchange;
use crate::formats::xls::{self, SheetReader, Cell, CellType};
use crate::instruments::InstrumentInfo;
use crate::time;
use crate::types::{Date, Time, Decimal};
use crate::util::{self, DecimalRestrictions};

#[derive(Default)]
pub struct SecurityInfo {
    pub isin: HashSet<ISIN>,
    pub symbols: HashSet<String>,
}

// Depending on when the statement has been generated it contains such information as instrument symbol and ISIN spread
// over different statement sections and the only way to find them is to join this information using instrument name as
// a key.
pub type SecuritiesRegistry = HashMap<String, SecurityInfo>;
pub type SecuritiesRegistryRc = Rc<RefCell<SecuritiesRegistry>>;

pub fn parse_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%d.%m.%Y")
}

pub fn parse_date_cell(cell: &Cell) -> GenericResult<Date> {
    parse_date(xls::get_string_cell(cell)?)
}

pub fn parse_planned_actual_date_cell(cell: &Cell) -> GenericResult<Date> {
    let value = xls::get_string_cell(cell)?;
    let values: Vec<&str> = value.split('/').collect();

    let date = match values.len() {
        1 => {
             // Deprecated case: old broker statements contain only one date
             values.first().unwrap()
        },
        2 => {
            let actual = values.last().unwrap();

            // For non-executed trades actual date is empty
            if actual.is_empty() {
                values.first().unwrap()
            } else {
                actual
            }
        },
        _ => return Err!("Invalid date: {:?}", value)
    };

    parse_date(date)
}

pub fn parse_time(time: &str) -> GenericResult<Time> {
    time::parse_time(time, "%H:%M:%S")
}

pub fn parse_time_cell(cell: &Cell) -> GenericResult<Time> {
    parse_time(xls::get_string_cell(cell)?)
}

pub fn parse_fractional_quantity_cell(cell: &Cell) -> GenericResult<Decimal> {
    parse_decimal_cell(cell).and_then(|quantity| {
        util::validate_decimal(quantity, DecimalRestrictions::PositiveOrZero)
    })
}

pub fn parse_decimal_cell(cell: &Cell) -> GenericResult<Decimal> {
    match cell {
        Cell::String(value) => {
            let value = value.replace(',', ".");
            util::parse_decimal(&value, DecimalRestrictions::No)
        },
        _ => Decimal::parse(cell, true),
    }
}

pub fn trim_column_title(title: &str) -> Cow<'_, str> {
    Cow::from(title.trim_end_matches('*')) // Footnotes
}

pub fn read_next_table_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
    let sheet_ptr = sheet as *mut SheetReader;

    {
        let row = sheet.next_row()?;

        let non_empty_cells = row.iter().filter(|cell| !matches!(cell, Cell::Empty)).count();
        if non_empty_cells > 1 {
            return Some(row);
        }
    }

    unsafe {
        &mut *sheet_ptr as &mut SheetReader
    }.step_back();

    None
}

pub fn save_instrument_exchange_info(instruments: &mut InstrumentInfo, symbol: &str, exchange: &str) -> EmptyResult {
    let exchange = match exchange {
        "ММВБ" | "МосБиржа" => Exchange::Moex,
        "СПБ" | "СПБиржа" => Exchange::Spb,
        "ВНБ" => Exchange::Otc, // https://github.com/KonishchevDmitry/investments/issues/82
        _ => return Err!("Unknown exchange: {:?}", exchange),
    };
    Ok(instruments.get_or_add(symbol).exchanges.add_prioritized(exchange))
}