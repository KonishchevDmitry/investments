use regex::{self, Regex};

use crate::core::GenericResult;

use super::{SheetReader, Cell, is_empty_row};

pub struct TableColumn {
    pub name: &'static str,
    pub optional: bool,
}

pub trait TableRow: Sized {
    fn columns() -> Vec<TableColumn>;
    fn parse(row: &[Option<&Cell>]) -> GenericResult<Self>;
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

    fn skip_row(_row: &[Option<&Cell>]) -> GenericResult<bool> {
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
    mapping: Vec<Option<usize>>,
}

impl ColumnsMapping {
    pub fn map<'a>(&self, row: &'a[Cell]) -> GenericResult<Vec<Option<&'a Cell>>> {
        let mut mapped_row = Vec::with_capacity(self.mapping.len());
        let mut current_cell_id = 0;

        for (column_id, &mapped_cell_id) in self.mapping.iter().enumerate() {
            let mapped_cell_id = match mapped_cell_id {
                Some(cell_id) => cell_id,
                None => {
                    mapped_row.push(None);
                    continue;
                },
            };

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

            mapped_row.push(Some(&row[mapped_cell_id]));
            current_cell_id = mapped_cell_id + 1;
        }

        let spare_cells = &row[current_cell_id..];
        if !is_empty_row(spare_cells) {
            return Err!("The row contains non-empty cells after column cells: {:?}", spare_cells);
        }

        Ok(mapped_row)
    }
}

pub fn map_columns(mut row: &[Cell], columns: &[TableColumn]) -> GenericResult<ColumnsMapping> {
    let mut mapping = Vec::with_capacity(columns.len());
    let mut offset = 0;

    for column in columns {
        let cell_id = match find_column(row, column.name, column.optional)? {
            Some(index) => {
                row = &row[index + 1..];
                let cell_id = offset + index;
                offset += index + 1;
                Some(cell_id)
            }
            None => None,
        };
        mapping.push(cell_id);
    }

    if !is_empty_row(row) {
        return Err!("The table has more columns than expected")
    }

    Ok(ColumnsMapping { mapping })
}

fn find_column(row: &[Cell], name: &str, optional: bool) -> GenericResult<Option<usize>> {
    for (cell_id, cell) in row.iter().enumerate() {
        match cell {
            Cell::String(value) => {
                let value_regex = format!("^{}$", regex::escape(value.trim()).replace("\n", " ?"));

                if Regex::new(&value_regex).unwrap().is_match(name) {
                    return Ok(Some(cell_id));
                } else if optional {
                    return Ok(None);
                } else {
                    return Err!("Unable to find {:?} column - got {:?} instead", name, value);
                }
            },
            Cell::Empty => {}
            _ => return Err!(
                "Unable to find {:?} column - got an unexpected {:?} cell", name, cell),
        };
    }

    if optional {
        Ok(None)
    } else {
        Err!("The table has no {:?} column", name)
    }
}