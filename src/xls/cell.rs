use crate::core::GenericResult;

pub use calamine::DataType as Cell;

pub trait CellType: Sized {
    fn parse(cell: &Cell) -> GenericResult<Self>;
}

impl CellType for String {
    fn parse(cell: &Cell) -> GenericResult<String> {
        // FIXME
        Ok(super::get_string_cell(cell)?.to_owned())
    }
}