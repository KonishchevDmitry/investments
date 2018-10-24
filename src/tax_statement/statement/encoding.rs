use core::GenericResult;

pub type Integer = usize;

pub trait TaxStatementType: Sized {
    fn decode(data: &str) -> GenericResult<Self>;
    fn encode(value: &Self) -> GenericResult<String>;
}

impl TaxStatementType for String {
    fn decode(data: &str) -> GenericResult<String> {
        Ok(data.to_owned())
    }

    fn encode(value: &String) -> GenericResult<String> {
        Ok(value.clone())
    }
}

impl TaxStatementType for usize {
    fn decode(data: &str) -> GenericResult<usize> {
        Ok(data.parse().map_err(|_| format!("Invalid usize value: {:?}", data))?)
    }

    fn encode(value: &usize) -> GenericResult<String> {
        Ok(value.to_string())
    }
}