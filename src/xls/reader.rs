use std::ops::Index;

use calamine::{Range, Reader, open_workbook_auto};

use crate::core::GenericResult;

use super::Cell;

pub struct SheetReader {
    sheet: Range<Cell>,
    next_row_id: usize,
}

impl SheetReader {
    pub fn new(path: &str, sheet_name: &str) -> GenericResult<SheetReader> {
        let mut workbook = open_workbook_auto(path)?;

        let sheet = workbook.worksheet_range(sheet_name).ok_or_else(|| format!(
            "There is no {:?} sheet in the workbook", sheet_name))??;

        Ok(SheetReader {
            sheet,
            next_row_id: 0,
        })
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
        while let Some(row) = self.next_row() {
            if !is_empty_row(row) {
                self.step_back();
                break;
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