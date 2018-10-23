use core::GenericResult;

use self::record::Record;
use self::parser::TaxStatementReader;

#[macro_use] mod record;
mod encoding;
mod foreign_income;
mod parser;

#[derive(Debug)]
pub struct TaxStatement {
    pub year: i32,
    records: Vec<Box<Record>>,
}

impl TaxStatement {
    pub fn read(path: &str) -> GenericResult<TaxStatement> {
        Ok(TaxStatementReader::read(path).map_err(|e| format!(
            "Error while reading {:?}: {}", path, e))?)
    }
}