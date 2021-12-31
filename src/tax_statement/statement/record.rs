use std::any::Any;
use std::fmt::Debug;

use crate::core::{EmptyResult, GenericResult};

use super::parser::{TaxStatementReader, TaxStatementWriter};

pub trait Record: Debug {
    fn name(&self) -> &str;
    fn as_mut_any(&mut self) -> &mut dyn Any;
    fn write(&self, writer: &mut TaxStatementWriter) -> EmptyResult;
}

#[derive(Debug)]
pub struct UnknownRecord {
    name: String,
    fields: Vec<String>,
}

impl UnknownRecord {
    pub fn read(reader: &mut TaxStatementReader, name: String) -> GenericResult<(UnknownRecord, Option<String>)> {
        let mut fields = Vec::new();
        let mut next_record_name = None;

        while !reader.at_eof()? {
            let data: String = reader.read_value()?;

            if is_record_name(&data) {
                next_record_name.replace(data);
                break;
            }

            fields.push(data);
        }

        let record = UnknownRecord {
            name: name,
            fields: fields,
        };

        Ok((record, next_record_name))
    }
}

impl Record for UnknownRecord {
    fn name(&self) -> &str {
        &self.name
    }

    fn as_mut_any(&mut self) -> &mut dyn Any {
        self
    }

    fn write(&self, writer: &mut TaxStatementWriter) -> EmptyResult {
        writer.write_data(&self.name)?;

        for field in &self.fields {
            writer.write_data(field)?;
        }

        Ok(())
    }
}

pub fn is_record_name(data: &str) -> bool {
    data.starts_with('@')
}

#[allow(unused_macros)]
macro_rules! tax_statement_record {
    (
        $name:ident {
            $($field_name:ident: $field_type:ty,)*
        }
    ) => {
        declare_tax_statement_record!($name {
            $($field_name: $field_type,)*
        });

        impl $name {
            pub fn read(
                reader: &mut $crate::tax_statement::statement::parser::TaxStatementReader
            ) -> $crate::core::GenericResult<$name> {
                Ok($name {
                    $($field_name: reader.read_value()?,)*
                })
            }
        }

        impl $crate::tax_statement::statement::record::Record for $name {
            fn name(&self) -> &str {
                $name::RECORD_NAME
            }

            fn as_mut_any(&mut self) -> &mut ::std::any::Any {
                self
            }

            fn write(
                &self, writer: &mut $crate::tax_statement::statement::parser::TaxStatementWriter,
            ) -> $crate::core::EmptyResult {
                writer.write_data($name::RECORD_NAME)?;
                $(writer.write_value(&self.$field_name)?;)*
                Ok(())
            }
        }
    }
}

macro_rules! tax_statement_inner_record {
    (
        $name:ident {
            $($field_name:ident: $field_type:ty,)*
        }
    ) => {
        declare_tax_statement_record_struct!($name {
            $($field_name: $field_type,)*
        });

        impl $crate::tax_statement::statement::encoding::TaxStatementType for $name {
            fn read(reader: &mut $crate::tax_statement::statement::parser::TaxStatementReader) -> GenericResult<$name> {
                Ok($name {
                    $(
                        $field_name: reader.read_value().map_err(|e| format!(
                            "{struct}::{field}: {error}",
                            struct=stringify!($name), field=stringify!($field_name), error=e))?,
                    )*
                })
            }

            fn write(&self, writer: &mut $crate::tax_statement::statement::parser::TaxStatementWriter) -> EmptyResult {
                $(writer.write_value(&self.$field_name)?;)*
                Ok(())
            }
        }
    }
}

macro_rules! tax_statement_array_record {
    (
        $name:ident {
            $($field_name:ident: $field_type:ty,)*
        }, index_length=$index_length:expr
    ) => {
        declare_tax_statement_record!($name {
            $($field_name: $field_type,)*
        });

        impl $name {
            pub fn read(
                reader: &mut $crate::tax_statement::statement::parser::TaxStatementReader,
                index: usize
            ) -> $crate::core::GenericResult<$name> {
                {
                    let name = $name::get_name(index)?;

                    let record_name = reader.read_data()?;
                    if record_name != name {
                        return Err!("Got {:?} where {} record is expected", record_name, name);
                    }
                }

                Ok($name {
                    $(
                        $field_name: reader.read_value().map_err(|e| format!(
                            "{struct}[{index}]::{field}: {error}",
                            struct=stringify!($name), index=index, field=stringify!($field_name),
                            error=e))?,
                    )*
                })
            }

            fn write(
                &self, writer: &mut $crate::tax_statement::statement::parser::TaxStatementWriter,
                index: usize
            ) -> $crate::core::EmptyResult {
                let name = $name::get_name(index)?;
                writer.write_data(&name)?;
                $(writer.write_value(&self.$field_name)?;)*
                Ok(())
            }

            fn get_name(index: usize) -> $crate::core::GenericResult<String> {
                use ::std::fmt::Write;

                let index_length = $index_length;
                let name_length = $name::RECORD_NAME.len() + index_length;

                let mut name = String::with_capacity(name_length);
                name.push_str($name::RECORD_NAME);
                write!(name, "{:0width$}", index, width=index_length).unwrap();

                if name.len() != name_length {
                    return Err!("Got a too big index for {} record: {}", $name::RECORD_NAME, index);
                }

                Ok(name)
            }
        }
    }
}

macro_rules! declare_tax_statement_record {
    (
        $name:ident {
            $($field_name:ident: $field_type:ty,)*
        }
    ) => {
        declare_tax_statement_record_struct!($name {
            $($field_name: $field_type,)*
        });

        impl $name {
            pub const RECORD_NAME: &'static str = concat!("@", stringify!($name));
        }
    }
}

macro_rules! declare_tax_statement_record_struct {
    (
        $name:ident {
            $($field_name:ident: $field_type:ty,)*
        }
    ) => {
        #[derive(Clone, PartialEq, Eq, Debug)]
        pub struct $name {
            $(pub $field_name: $field_type,)*
        }
    }
}
