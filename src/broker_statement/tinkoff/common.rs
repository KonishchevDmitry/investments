use crate::core::GenericResult;
use crate::currency::Cash;
use crate::time;
use crate::types::{Date, Time, Decimal};
use crate::util::{self, DecimalRestrictions};
use crate::xls::{SheetReader, Cell};

pub fn parse_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%d.%m.%Y")
}

pub fn parse_time(time: &str) -> GenericResult<Time> {
    time::parse_time(time, "%H:%M:%S")
}

pub fn parse_decimal(string: &str, restrictions: DecimalRestrictions) -> GenericResult<Decimal> {
    util::parse_decimal(&string.replace(',', "."), restrictions)
}

pub fn parse_cash(currency: &str, value: &str, restrictions: DecimalRestrictions) -> GenericResult<Cash> {
    Ok(Cash::new(currency, parse_decimal(value, restrictions)?))
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