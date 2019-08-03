#![recursion_limit="128"]

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
    alignment: Option<String>,
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
    let span = Span::call_site();

    let table_name = get_table_params(&ast)?;
    let columns = get_table_columns(&ast)?;

    let mod_ident = quote!(crate::formatting::table);
    let table_ident = Ident::new(&table_name, span);
    let row_proxy_ident = Ident::new(&(table_name + "RowProxy"), span);
    let row_ident = &ast.ident;

    let field_idents = columns.iter().map(|column| {
        Ident::new(&column.id, span)
    });

    let columns_init_code = columns.iter().map(|column| {
        let name = column.name.as_ref().unwrap_or(&column.id);
        let alignment = match column.alignment {
            Some(ref alignment) => {
                let alignment_ident = Ident::new(&alignment.to_uppercase(), span);
                quote!(Some(#mod_ident::Alignment::#alignment_ident))
            },
            None => quote!(None),
        };
        quote! {
            #mod_ident::Column::new(#name, #alignment)
        }
    });

    let column_hide_code = columns.iter().enumerate().map(|(index, column)| {
        let method_ident = Ident::new(&format!("hide_{}", column.id), span);
        quote! {
            fn #method_ident(&mut self) {
                self.table.hide_column(#index);
            }
        }
    });

    let cell_set_code = columns.iter().enumerate().map(|(index, column)| {
        let method_ident = Ident::new(&format!("set_{}", column.id), span);
        quote! {
            fn #method_ident(&mut self, cell: #mod_ident::Cell) {
                self.row[#index] = cell;
            }
        }
    });

    Ok(quote! {
        struct #table_ident {
            table: #mod_ident::Table,
        }

        impl #table_ident {
            fn new() -> #table_ident {
                let columns = vec![#(#columns_init_code,)*];
                #table_ident {
                    table: #mod_ident::Table::new(columns),
                }
            }

            fn add_row(&mut self, row: #row_ident) -> #row_proxy_ident {
                let row = self.table.add_row(row.into());
                #row_proxy_ident {row: row}
            }

            #(#column_hide_code)*

            fn print(&self, title: &str) {
                self.table.print(title);
            }
        }

        struct #row_proxy_ident<'a> {
            row: &'a mut #mod_ident::Row,
        }

        impl<'a> #row_proxy_ident<'a> {
            #(#cell_set_code)*
        }

        impl<'a, 'b> ::core::iter::IntoIterator for &'a mut #row_proxy_ident<'b> {
            type Item = &'a mut #mod_ident::Cell;
            type IntoIter = ::std::slice::IterMut<'a, #mod_ident::Cell>;

            fn into_iter(self) -> Self::IntoIter {
                self.row.iter_mut()
            }
        }

        impl ::std::convert::Into<#mod_ident::Row> for #row_ident {
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
    #[derive(FromMeta, Default)]
    struct ColumnParams {
        #[darling(default)]
        name: Option<String>,
        #[darling(default)]
        align: Option<String>,
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

            match params.align.as_ref().map(|value| value.as_str()) {
                Some("left") | Some("center") | Some("right") | None => {},
                _ => return Err!("Invalid alignment of {:?}: {:?}",
                                 field_name, params.align.unwrap()),
            };

            if field_params.replace(params).is_some() {
                return Err!("Duplicated {:?} attribute on {:?} field", COLUMN_ATTR_NAME, field_name);
            }
        }

        let column_params = field_params.unwrap_or_default();
        columns.push(Column {
            id: field_name,
            name: column_params.name,
            alignment: column_params.align,
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