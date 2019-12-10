use std::ops::Index;

use calamine::{Range, Reader, Xls, open_workbook};
use regex::{self, Regex};

use crate::core::GenericResult;

pub use calamine::DataType as Cell;

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

pub struct ColumnsMapping {
    mapping: Vec<usize>,
}

impl ColumnsMapping {
    pub fn map<'a>(&self, row: &'a[Cell]) -> GenericResult<Vec<&'a Cell>> {
        let mut mapped_row = Vec::with_capacity(self.mapping.len());
        let mut current_cell_id = 0;

        for (column_id, &mapped_cell_id) in self.mapping.iter().enumerate() {
            if mapped_cell_id >= row.len() {
                return Err!("The row have no {}:{} column", column_id, mapped_cell_id);
            }

            if current_cell_id < mapped_cell_id {
                let spare_cells = &row[current_cell_id..mapped_cell_id];
                if !is_empty_row(spare_cells) {
                    return Err!(
                        "The row contains non-empty cells between column cells: {:?}", spare_cells);
                }
            } else {
                assert_eq!(current_cell_id, mapped_cell_id);
            }

            mapped_row.push(&row[mapped_cell_id]);
            current_cell_id = mapped_cell_id + 1;
        }

        let spare_cells = &row[current_cell_id..];
        if !is_empty_row(spare_cells) {
            return Err!("The row contains non-empty cells after column cells: {:?}", spare_cells);
        }

        Ok(mapped_row)
    }
}

pub fn map_columns(mut row: &[Cell], columns: &[&str]) -> GenericResult<ColumnsMapping> {
    let mut mapping = Vec::with_capacity(columns.len());
    let mut offset = 0;

    for column_name in columns {
        let cell_id = find_column(row, column_name)?;
        row = &row[cell_id + 1..];

        mapping.push(offset + cell_id);
        offset += cell_id + 1;
    }

    if !is_empty_row(row) {
        return Err!("The table has more columns than expected")
    }

    Ok(ColumnsMapping { mapping })
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

pub fn get_string_cell(cell: &Cell) -> GenericResult<&str> {
    match cell {
        Cell::String(value) => Ok(value),
        _ => Err!("Got an unexpected cell value where string is expected: {:?}", cell),
    }
}

fn find_column(row: &[Cell], name: &str) -> GenericResult<usize> {
    for (cell_id, cell) in row.iter().enumerate() {
        match cell {
            Cell::String(value) => {
                let value_regex = format!("^{}$", regex::escape(value).replace("\n", " ?"));

                if Regex::new(&value_regex).unwrap().is_match(name) {
                    return Ok(cell_id);
                } else {
                    return Err!("Unable to find {:?} column - got {:?} instead", name, value);
                }
            },
            Cell::Empty => {}
            _ => return Err!(
                "Unable to find {:?} column - got an unexpected {:?} cell", name, cell),
        };
    }

    return Err!("The table has no {:?} column", name);
}