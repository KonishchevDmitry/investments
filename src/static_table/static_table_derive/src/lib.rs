extern crate proc_macro;

use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{self, DeriveInput, Fields, Data, DataStruct, Meta, MetaList, MetaNameValue};

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type GenericResult<T> = Result<T, GenericError>;

#[cfg_attr(test, derive(Debug))]
struct Column {
    id: String,
    name: Option<String>,
}

const TABLE_ATTR_NAME: &str = "table";
const COLUMN_ATTR_NAME: &str = "column";

macro_rules! Err {
    ($($arg:tt)*) => (::std::result::Result::Err(format!($($arg)*).into()))
}

#[proc_macro_derive(StaticTable, attributes(table, column))]
pub fn static_table_derive(input: TokenStream) -> TokenStream {
    match static_table_derive_impl(input) {
        Ok(output) => output,
        Err(err) => panic!("{}", err),
    }
}

fn static_table_derive_impl(input: TokenStream) -> GenericResult<TokenStream> {
    let ast: DeriveInput = syn::parse(input)?;
    let table_name = get_table_params(&ast)?;
    let columns = get_table_columns(&ast)?;

    let mod_ident = quote!(crate::static_table);
    let table_ident = Ident::new(&table_name, Span::call_site());
    let row_ident = &ast.ident;

    let field_idents = columns.iter().map(|column| {
        Ident::new(&column.id, Span::call_site())
    });

    let columns_init_code = columns.iter().map(|column| {
        let id = &column.id;
        let name = match column.name {
            Some(ref name) => quote!(Some(#name)),
            None => quote!(None),
        };

        quote! {
            #mod_ident::Column {
                id: #id,
                name: #name,
            }
        }
    });

    Ok(quote! {
        struct #table_ident {
            raw_table: #mod_ident::Table,
        }

        impl #table_ident {
            fn new() -> #table_ident {
                #table_ident {
                    raw_table: #mod_ident::Table {
                        columns: vec![#(#columns_init_code,)*],
                        rows: Vec::new(),
                    }
                }
            }

            fn add_row(&mut self, row: #row_ident) {
                self.raw_table.add_row(row.into());
            }
        }

        impl Into<#mod_ident::Row> for #row_ident {
            fn into(self) -> #mod_ident::Row {
                vec![#(self.#field_idents.into(),)*]
            }
        }
    }.into())
}

fn get_table_params(ast: &DeriveInput) -> GenericResult<String> {
    #[derive(FromMeta)]
    struct TableParams {
        name: String,
    }

    let mut table_name = None;

    for attr in &ast.attrs {
        let meta = attr.parse_meta().map_err(|e| format!(
            "Failed to parse `{:#?}`: {}", attr, e))?;

        let ident = get_attribute_ident(&meta);
        if ident == COLUMN_ATTR_NAME {
            return Err!("{:?} attribute is allowed on struct fields only", COLUMN_ATTR_NAME);
        } else if ident != TABLE_ATTR_NAME {
            continue;
        }

        let params = TableParams::from_meta(&meta).map_err(|e| format!(
            "{:?} attribute validation error: {}", TABLE_ATTR_NAME, e))?;

        if table_name.replace(params.name).is_some() {
            return Err!("Duplicated {:?} attribute", TABLE_ATTR_NAME);
        }
    }

    Ok(table_name.unwrap_or_else(|| String::from("Table")))
}

fn get_table_columns(ast: &DeriveInput) -> GenericResult<Vec<Column>> {
    #[derive(FromMeta)]
    struct ColumnParams {
        #[darling(default)]
        name: Option<String>,
    }

    let mut columns = Vec::new();

    let fields = match ast.data {
        Data::Struct(DataStruct{fields: Fields::Named(ref fields), ..}) => &fields.named,
        _ => return Err!("A struct with named fields is expected"),
    };

    for field in fields {
        let field_name = field.ident.as_ref()
            .ok_or_else(|| "A struct with named fields is expected")?.to_string();
        let mut field_params = None;

        for attr in &field.attrs {
            let meta = attr.parse_meta().map_err(|e| format!(
                "Failed to parse `{:#?}` on {:?} field: {}", attr, field_name, e))?;

            let ident = get_attribute_ident(&meta);
            if ident == TABLE_ATTR_NAME {
                return Err!("{:?} attribute is allowed on struct definition only", TABLE_ATTR_NAME);
            } else if ident != COLUMN_ATTR_NAME {
                continue;
            }

            let params = ColumnParams::from_meta(&meta).map_err(|e| format!(
                "{:?} attribute on {:?} field validation error: {}", COLUMN_ATTR_NAME, field_name, e))?;

            if field_params.replace(params).is_some() {
                return Err!("Duplicated {:?} attribute on {:?} field", COLUMN_ATTR_NAME, field_name);
            }
        }

        let column_params = field_params.unwrap_or_else(|| ColumnParams {
            name: None,
        });

        columns.push(Column {
            id: field_name,
            name: column_params.name,
        })
    }

    Ok(columns)
}

fn get_attribute_ident(meta: &Meta) -> &Ident {
    match meta {
        Meta::Word(ident) => ident,
        Meta::List(MetaList{ident, ..}) => ident,
        Meta::NameValue(MetaNameValue{ident, ..}) => ident,
    }
}