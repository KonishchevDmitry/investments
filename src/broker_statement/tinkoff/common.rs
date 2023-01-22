use crate::core::{EmptyResult, GenericResult};
use crate::exchanges::Exchange;
use crate::instruments::InstrumentInfo;
use crate::time;
use crate::types::{Date, Time, Decimal};
use crate::util::{self, DecimalRestrictions};
use crate::xls::{self, SheetReader, Cell, CellType};

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

pub fn parse_quantity_cell(cell: &Cell) -> GenericResult<u32> {
    let value = xls::get_string_cell(cell)?;
    Ok(value.parse().map_err(|_| format!("Invalid quantity: {}", value))?)
}

pub fn parse_decimal_cell(cell: &Cell) -> GenericResult<Decimal> {
    match cell {
        Cell::String(value) => {
            let value = value.replace(',', ".");
            util::parse_decimal(&value, DecimalRestrictions::No)
        },
        _ => Decimal::parse(cell),
    }
}

pub fn read_next_table_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
    let sheet_ptr = sheet as *mut SheetReader;

    {
        let row = match sheet.next_row() {
            Some(row) => row,
            None => return None,
        };

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
        _ => return Err!("Unknown exchange: {:?}", exchange),
    };
    Ok(instruments.get_or_add(symbol).exchanges.add_prioritized(exchange))
}