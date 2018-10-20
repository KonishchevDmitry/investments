use std::fmt::Debug;

use core::GenericResult;

use super::TaxStatementParser;

pub trait Record: Debug {
}

pub type ParseResult = GenericResult<(Box<Record>, Option<String>)>;

#[derive(Debug)]
pub struct UnknownRecord {
    name: String,
    fields: Vec<String>,
}

impl UnknownRecord {
    pub fn parse(parser: &mut TaxStatementParser, name: String) -> ParseResult {
        let mut fields = Vec::new();

        loop {
            let data: String = parser.read_type()?;

            if is_record_name(&data) {
                let record = UnknownRecord {
                    name: name,
                    fields: fields,
                };
                return Ok((Box::new(record), Some(data)));
            }

            fields.push(data);
        }
    }
}

impl Record for UnknownRecord {
}

pub fn is_record_name(data: &str) -> bool {
    data.starts_with('@')
}
