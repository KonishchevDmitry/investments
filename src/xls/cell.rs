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

macro_rules! impl_integer_parser {
    ($name:ident) => {
        impl CellType for $name {
            fn parse(cell: &Cell) -> GenericResult<$name> {
                parse_integer(cell, stringify!($name))
            }
        }
    }
}
impl_integer_parser!(i32);
impl_integer_parser!(u32);

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

pub fn parse_with<T, C>(cell: &Cell, parse: fn(&Cell) -> GenericResult<T>) -> GenericResult<C>
    where C: FromParsedOptional<T>
{
    FromParsedOptional::from_parsed_optional(match cell {
        Cell::Empty => None,
        _ => Some(parse(cell)?),
    })
}

pub trait FromParsedOptional<T>: Sized {
    fn from_parsed_optional(value: Option<T>) -> GenericResult<Self>;
}

impl<T> FromParsedOptional<T> for Option<T> {
    fn from_parsed_optional(value: Option<T>) -> GenericResult<Option<T>> {
        Ok(value)
    }
}

impl<T> FromParsedOptional<T> for T {
    fn from_parsed_optional(value: Option<T>) -> GenericResult<T> {
        match value {
            Some(value) => Ok(value),
            None => return Err!("Got an empty cell"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_using_parse_with() {
        let value: i64 = parse_with(&Cell::Int(42), parse_int).unwrap();
        assert_eq!(value, 42);

        match parse_with(&Cell::Empty, parse_int) {
            Ok(value) => {
                let _: i64 = value;
                unreachable!()
            },
            Err(e) => assert_eq!(e.to_string(), "Got an empty cell"),
        };

        let optional_value: Option<i64> = parse_with(&Cell::Int(42), parse_int).unwrap();
        assert_eq!(optional_value, Some(42));

        let optional_value: Option<i64> = parse_with(&Cell::Empty, parse_int).unwrap();
        assert_eq!(optional_value, None);
    }

    fn parse_int(cell: &Cell) -> GenericResult<i64> {
        match cell {
            Cell::Int(value) => Ok(*value),
            _ => return Err!("Invalid cell value: {:?}", cell),
        }
    }
}