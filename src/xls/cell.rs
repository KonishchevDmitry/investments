use crate::core::GenericResult;

pub use calamine::DataType as Cell;

pub fn get_string_cell(cell: &Cell) -> GenericResult<&str> {
    match cell {
        Cell::String(value) => Ok(value),
        _ => Err!("Got an unexpected cell value where string is expected: {:?}", cell),
    }
}

pub trait CellType: Sized {
    fn parse(cell: &Cell) -> GenericResult<Self>;
}

impl CellType for String {
    fn parse(cell: &Cell) -> GenericResult<String> {
        Ok(get_string_cell(cell)?.to_owned())
    }
}

#[derive(Debug)]
pub struct SkipCell {
}

impl CellType for SkipCell {
    fn parse(_: &Cell) -> GenericResult<SkipCell> {
        Ok(SkipCell {})
    }
}