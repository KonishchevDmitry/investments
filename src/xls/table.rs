use regex::{self, Regex};

use crate::core::GenericResult;

use super::{SheetReader, Cell, is_empty_row};

pub trait TableRow: Sized {
    fn columns() -> Vec<&'static str>;
    fn parse(row: &[&Cell]) -> GenericResult<Self>;
}

pub trait TableReader {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        sheet.next_row().and_then(|row| {
            if is_empty_row(row) {
                None
            } else {
                Some(row)
            }
        })
    }

    fn skip_row(_row: &[&Cell]) -> GenericResult<bool> {
        Ok(false)
    }
}

pub fn read_table<T: TableRow + TableReader>(sheet: &mut SheetReader) -> GenericResult<Vec<T>> {
    let columns = T::columns();
    let columns_mapping = map_columns(sheet.next_row_checked()?, &columns)?;

    let mut table = Vec::new();

    while let Some(row) = T::next_row(sheet) {
        let mapped_row = columns_mapping.map(row)?;
        if T::skip_row(&mapped_row)? {
            continue;
        }

        table.push(TableRow::parse(&mapped_row)?);
    }

    Ok(table)
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

fn find_column(row: &[Cell], name: &str) -> GenericResult<usize> {
    for (cell_id, cell) in row.iter().enumerate() {
        match cell {
            Cell::String(value) => {
                let value_regex = format!("^{}$", regex::escape(value.trim()).replace("\n", " ?"));

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