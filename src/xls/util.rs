use crate::core::GenericResult;

use super::Cell;

pub fn is_empty_row(row: &[Cell]) -> bool {
    row.iter().all(|cell| matches!(cell, Cell::Empty))
}

#[allow(dead_code)]
fn trim_row(row: &[Cell]) -> &[Cell] {
    trim_row_right(trim_row_left(row))
}

pub fn trim_row_left(mut row: &[Cell]) -> &[Cell] {
    while let Some(Cell::Empty) = row.first() {
        row = &row[1..]
    }
    row
}

pub fn trim_row_right(mut row: &[Cell]) -> &[Cell] {
    while let Some(Cell::Empty) = row.last() {
        row = &row[..row.len() - 1]
    }
    row
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