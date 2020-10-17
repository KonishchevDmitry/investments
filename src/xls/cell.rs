use num_traits::cast::FromPrimitive;

use crate::core::GenericResult;
use crate::types::Decimal;

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

#[derive(Debug)]
pub struct SkipCell {
}

impl CellType for SkipCell {
    fn parse(_: &Cell) -> GenericResult<SkipCell> {
        Ok(SkipCell {})
    }
}

impl CellType for String {
    fn parse(cell: &Cell) -> GenericResult<String> {
        Ok(get_string_cell(cell)?.to_owned())
    }
}

impl CellType for i32 {
    fn parse(cell: &Cell) -> GenericResult<i32> {
        parse_integer(cell, "i32")
    }
}

impl CellType for u32 {
    fn parse(cell: &Cell) -> GenericResult<u32> {
        parse_integer(cell, "u32")
    }
}

fn parse_integer<I>(cell: &Cell, type_name: &str) -> GenericResult<I> where I: FromPrimitive {
    Ok(match cell {
        Cell::Int(value) => I::from_i64(*value),
        Cell::Float(value) => {
            if value.fract() == 0.0 {
                I::from_f64(*value)
            } else {
                None
            }
        }
        _ => None,
    }.ok_or_else(|| format!(
        "Got an unexpected cell value where {} is expected: {:?}", type_name, cell
    ))?)
}

impl CellType for Decimal {
    fn parse(cell: &Cell) -> GenericResult<Decimal> {
        Ok(match cell {
            Cell::Float(value) => Decimal::from_f64(*value),
            Cell::Int(value) => Decimal::from_i64(*value),
            _ => None,
        }.ok_or_else(|| format!(
            "Got an unexpected cell value where decimal is expected: {:?}", cell
        ))?)
    }
}

impl<T: CellType> CellType for Option<T> {
    fn parse(cell: &Cell) -> GenericResult<Option<T>> {
        match cell {
            Cell::Empty => Ok(None),
            _ => Ok(Some(CellType::parse(cell)?)),
        }
    }
}