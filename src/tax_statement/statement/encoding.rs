use std::fmt::Write;

use core::{EmptyResult, GenericResult};

pub type Integer = usize;

pub trait TaxStatementType: Sized {
    fn decode(data: &str) -> GenericResult<Self>;
    fn encode(value: &Self, buffer: &mut String) -> EmptyResult;
}

impl TaxStatementType for String {
    fn decode(data: &str) -> GenericResult<String> {
        Ok(data.to_owned())
    }

    fn encode(value: &String, buffer: &mut String) -> EmptyResult {
        Ok(buffer.push_str(&value))
    }
}

impl TaxStatementType for usize {
    fn decode(data: &str) -> GenericResult<usize> {
        Ok(data.parse().map_err(|_| format!("Invalid usize value: {:?}", data))?)
    }

    fn encode(value: &usize, buffer: &mut String) -> EmptyResult {
        Ok(write!(buffer, "{}", value)?)
    }
}