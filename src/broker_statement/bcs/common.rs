use calamine::DataType;

use crate::core::GenericResult;
use crate::types::Date;
use crate::util;

pub fn parse_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%d.%m.%Y")
}

pub fn strip_row_expecting_columns(row: &[DataType], columns: usize) -> GenericResult<Vec<&DataType>> {
    let mut stripped = Vec::with_capacity(columns);

    for cell in row {
        match cell {
            DataType::Empty => {},
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

pub fn get_string_cell(cell: &DataType) -> GenericResult<&str> {
    match cell {
        DataType::String(value) => Ok(value),
        _ => Err!("Got an unexpected cell value where string is expected: {:?}", cell),
    }
}