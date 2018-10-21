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
            let data: String = parser.read_value()?;

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

macro_rules! tax_statement_record {
    (
        $name:ident {
            $($field_name:ident: $field_type:ty,)*
        }
    ) => {
        #[derive(Debug)]
        struct $name {
            $($field_name: $field_type,)*
        }

        impl $name {
            fn parse(parser: &mut TaxStatementParser) -> GenericResult<$name> {
                Ok($name {
                    $($field_name: parser.read_value()?,)*
                })
            }
        }
    }
}

pub fn is_record_name(data: &str) -> bool {
    data.starts_with('@')
}
