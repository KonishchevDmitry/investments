pub mod cell;
pub mod table;

use std::ops::Index;

use calamine::{Range, Reader, Xls, open_workbook};

use crate::core::GenericResult;

pub use self::cell::*;
pub use self::table::*;

pub struct SheetReader {
    sheet: Range<Cell>,
    next_row_id: usize,
}

impl SheetReader {
    pub fn new(path: &str, sheet_name: &str) -> GenericResult<SheetReader> {
        let mut workbook: Xls<_> = open_workbook(path)?;

        let sheet = workbook.worksheet_range(sheet_name).ok_or_else(|| format!(
            "There is no {:?} sheet in the workbook", sheet_name))??;

        Ok(SheetReader {
            sheet,
            next_row_id: 0,
        })
    }

    // FIXME: Do we need it with step_back()?
    pub fn peek_row(&mut self) -> Option<&[Cell]> {
        if self.next_row_id < self.sheet.height() {
            Some(self.sheet.index(self.next_row_id))
        } else {
            None
        }
    }

    pub fn next_row(&mut self) -> Option<&[Cell]> {
        if self.next_row_id < self.sheet.height() {
            let row = self.sheet.index(self.next_row_id);
            self.next_row_id += 1;
            Some(row)
        } else {
            None
        }
    }

    pub fn next_row_checked(&mut self) -> GenericResult<&[Cell]> {
        Ok(self.next_row().ok_or_else(|| "Got an unexpected end of sheet")?)
    }

    pub fn step_back(&mut self) {
        assert!(self.next_row_id > 0);
        self.next_row_id -= 1;
    }

    pub fn skip_empty_rows(&mut self) {
        while let Some(row) = self.peek_row() {
            if is_empty_row(row) {
                self.next_row_id += 1;
            } else {
                break;
            }
        }
    }

    pub fn skip_non_empty_rows(&mut self) {
        while let Some(row) = self.peek_row() {
            if is_empty_row(row) {
                break;
            } else {
                self.next_row_id += 1;
            }
        }
    }
}

pub fn is_empty_row(row: &[Cell]) -> bool {
    row.iter().all(|cell| {
        if let Cell::Empty = cell {
            true
        } else {
            false
        }
    })
}

pub fn strip_row_expecting_columns(row: &[Cell], columns: usize) -> GenericResult<Vec<&Cell>> {
    let mut stripped = Vec::with_capacity(columns);

    for cell in row {
        match cell {
            Cell::Empty => {},
            _ => stripped.push(cell),
        };
    }

    if stripped.len() != columns {
        return Err!(
            "Got an unexpected number of non-empty columns in row: {} instead of {}",
            stripped.len(), columns);
    }

    Ok(stripped)
}