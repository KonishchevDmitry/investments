extern crate proc_macro;

use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::{self, DeriveInput, Fields, Data, Meta, MetaList, MetaNameValue, Ident};

// FIXME
type EmptyResult = GenericResult<()>;
type GenericResult<T> = Result<T, GenericError>;
type GenericError = Box<::std::error::Error + Send + Sync>;
macro_rules! Err {
    ($($arg:tt)*) => (::std::result::Result::Err(format!($($arg)*).into()))
}

struct Column {
    id: String,
    name: Option<String>,
}

const TABLE_ATTR_NAME: &str = "table";
const CELL_ATTR_NAME: &str = "cell";

#[proc_macro_derive(StaticTable, attributes(table, cell))]
pub fn static_table_derive(input: TokenStream) -> TokenStream {
    match static_table_derive_impl(input) {
        Ok(output) => output,
        Err(err) => panic!("{}", err),
    }
}

fn static_table_derive_impl(input: TokenStream) -> GenericResult<TokenStream> {
    let ast: DeriveInput = syn::parse(input)?;

    // FIXME: HERE
    let table_name = get_table_params(&ast)?;
    let columns = get_table_columns(&ast)?;

    let name = &ast.ident;
    let gen = quote! {
        impl HelloMacro for #name {
            fn hello_macro() {
                println!("Hello, Macro! My name is {}", stringify!(#name));
            }
        }
    };

    Ok(gen.into())
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
        if ident == CELL_ATTR_NAME {
            return Err!("{:?} attribute is allowed on struct fields only", CELL_ATTR_NAME);
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
        Data::Struct(ref row_struct) => {
            match row_struct.fields {
                Fields::Named(ref fields) => fields,
                _ => return Err!("A struct with named fields is expected"),
            }
        },
        _ => return Err!("A struct is expected"),
    };

    for field in &fields.named {
        let field_name = field.ident.as_ref()
            .ok_or_else(|| "A struct with named fields is expected")?.to_string();
        let mut field_params = None;

        for attr in &field.attrs {
            let meta = attr.parse_meta().map_err(|e| format!(
                "Failed to parse `{:#?}` on {:?} field: {}", attr, field_name, e))?;

            let ident = get_attribute_ident(&meta);
            if ident == TABLE_ATTR_NAME {
                return Err!("{:?} attribute is allowed on struct definition only", TABLE_ATTR_NAME);
            } else if ident != CELL_ATTR_NAME {
                continue;
            }

            let params = ColumnParams::from_meta(&meta).map_err(|e| format!(
                "{:?} attribute on {:?} field validation error: {}", CELL_ATTR_NAME, field_name, e))?;

            if field_params.replace(params).is_some() {
                return Err!("Duplicated {:?} attribute on {:?} field", CELL_ATTR_NAME, field_name);
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