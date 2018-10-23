use core::GenericResult;

pub type Integer = usize;

pub trait TaxStatementType: Sized {
    fn decode(data: &str) -> GenericResult<Self>;
}

impl TaxStatementType for String {
    fn decode(data: &str) -> GenericResult<String> {
        Ok(data.to_owned())
    }
}

impl TaxStatementType for usize {
    fn decode(data: &str) -> GenericResult<usize> {
        Ok(data.parse().map_err(|_| format!("Invalid usize value: {:?}", data))?)
    }
}