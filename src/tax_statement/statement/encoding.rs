use std::fmt::Write;
use std::str::FromStr;

use chrono::Duration;

use crate::core::{EmptyResult, GenericResult};
use crate::currency;
use crate::types::{Date, Decimal};

use super::parser::{TaxStatementReader, TaxStatementWriter};

pub trait TaxStatementType: Sized {
    fn read(reader: &mut TaxStatementReader) -> GenericResult<Self>;
    fn write(&self, writer: &mut TaxStatementWriter) -> EmptyResult;
}

pub trait TaxStatementPrimitiveType: Sized {
    fn decode(data: &str) -> GenericResult<Self>;
    fn encode(&self, buffer: &mut String) -> EmptyResult;
}

macro_rules! impl_tax_statement_type {
    ($name:ident) => {
        impl TaxStatementType for $name {
            fn read(reader: &mut TaxStatementReader) -> GenericResult<$name> {
                reader.read_primitive()
            }

            fn write(&self, writer: &mut TaxStatementWriter) -> EmptyResult {
                writer.write_primitive(self)
            }
        }
    }
}

impl_tax_statement_type!(usize);
impl TaxStatementPrimitiveType for usize {
    fn decode(data: &str) -> GenericResult<usize> {
        Ok(data.parse().map_err(|_| format!("Invalid integer value: {:?}", data))?)
    }

    fn encode(&self, buffer: &mut String) -> EmptyResult {
        Ok(write!(buffer, "{}", self)?)
    }
}

impl_tax_statement_type!(bool);
impl TaxStatementPrimitiveType for bool {
    fn decode(data: &str) -> GenericResult<bool> {
        Ok(match data {
            "0" => false,
            "1" => true,
            _ => return Err!("Invalid boolean value: {:?}", data),
        })
    }

    fn encode(&self, buffer: &mut String) -> EmptyResult {
        Ok(buffer.push(match self {
            false => '0',
            true => '1',
        }))
    }
}

impl_tax_statement_type!(String);
impl TaxStatementPrimitiveType for String {
    fn decode(data: &str) -> GenericResult<String> {
        Ok(data.to_owned())
    }

    fn encode(&self, buffer: &mut String) -> EmptyResult {
        Ok(buffer.push_str(self))
    }
}

impl_tax_statement_type!(Date);
impl TaxStatementPrimitiveType for Date {
    fn decode(data: &str) -> GenericResult<Date> {
        let days = Duration::days(data.parse().map_err(|_| format!(
            "Invalid integer value: {:?}", data))?);

        Ok(get_base_date() + days)
    }

    fn encode(&self, buffer: &mut String) -> EmptyResult {
        let days = (*self - get_base_date()).num_days();
        Ok(write!(buffer, "{}", days)?)
    }
}

fn get_base_date() -> Date {
    date!(1899, 12, 30)
}

impl_tax_statement_type!(Decimal);
impl TaxStatementPrimitiveType for Decimal {
    fn decode(data: &str) -> GenericResult<Decimal> {
        Ok(Decimal::from_str(data).map_err(|_| format!("Invalid decimal value: {:?}", data))?)
    }

    fn encode(&self, buffer: &mut String) -> EmptyResult {
        let value = currency::round(*self);

        if value != *self {
            return Err!("An attempt to write a non-rounded decimal value: {:?}", self);
        }

        Ok(write!(buffer, "{}", value)?)
    }
}