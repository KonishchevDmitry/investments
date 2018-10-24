use std::fmt::Debug;

use core::{EmptyResult, GenericResult};

use super::parser::{TaxStatementReader, TaxStatementWriter};

pub trait Record: Debug {
    fn write(&self, writer: &mut TaxStatementWriter) -> EmptyResult;
}

#[derive(Debug)]
pub struct UnknownRecord {
    name: String,
    fields: Vec<String>,
}

impl UnknownRecord {
    pub fn read(reader: &mut TaxStatementReader, name: String) -> GenericResult<(UnknownRecord, String)> {
        let mut fields = Vec::new();

        loop {
            let data: String = reader.read_value()?;

            if is_record_name(&data) {
                let record = UnknownRecord {
                    name: name,
                    fields: fields,
                };
                return Ok((record, data));
            }

            fields.push(data);
        }
    }
}

impl Record for UnknownRecord {
    fn write(&self, writer: &mut TaxStatementWriter) -> EmptyResult {
        writer.write_data(&self.name)?;

        for field in &self.fields {
            writer.write_data(field)?;
        }

        Ok(())
    }
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
            pub const RECORD_NAME: &'static str = concat!("@", stringify!($name));

            pub fn read(reader: &mut $crate::tax_statement::statement::parser::TaxStatementReader) -> $crate::core::GenericResult<$name> {
                Ok($name {
                    $($field_name: reader.read_value()?,)*
                })
            }
        }

        impl $crate::tax_statement::statement::record::Record for $name {
            fn write(&self, writer: &mut $crate::tax_statement::statement::parser::TaxStatementWriter) -> $crate::core::EmptyResult {
                writer.write_data($name::RECORD_NAME)?;
                $(writer.write_value(&self.$field_name)?;)*
                Ok(())
            }
        }
    }
}

pub fn is_record_name(data: &str) -> bool {
    data.starts_with('@')
}
