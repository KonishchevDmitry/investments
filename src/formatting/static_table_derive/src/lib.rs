#![recursion_limit="128"]

use darling::FromMeta;
use darling::ast::NestedMeta;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{self, DeriveInput, Fields, Data, DataStruct};

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type GenericResult<T> = Result<T, GenericError>;

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

    let table_name = TableParams::parse(&ast)?.name;
    let columns = Column::parse(&ast)?;

    let mod_ident = quote!(crate::formatting::table);
    let table_ident = Ident::new(&table_name, span);
    let row_proxy_ident = Ident::new(&(table_name + "RowProxy"), span);
    let row_ident = &ast.ident;

    let field_idents = columns.iter().map(|column| {
        Ident::new(&column.id, span)
    });

    let columns_init_code = columns.iter().map(|column| {
        let name = column.name.as_ref().unwrap_or(&column.id);
        match column.alignment {
            Some(ref alignment) => {
                let alignment_ident = Ident::new(&alignment.to_uppercase(), span);
                quote!(#mod_ident::Column::new_aligned(
                    #name, #mod_ident::Alignment::#alignment_ident))
            },
            None => quote!(#mod_ident::Column::new(#name))
        }
    });

    let column_modify_code = columns.iter().enumerate().map(|(index, column)| {
        let rename_method_ident = Ident::new(&format!("rename_{}", column.id), span);
        let hide_method_ident = Ident::new(&format!("hide_{}", column.id), span);
        quote! {
            fn #rename_method_ident(&mut self, name: &'static str) {
                self.table.rename_column(#index, name);
            }

            fn #hide_method_ident(&mut self) {
                self.table.hide_column(#index);
            }
        }
    });

    let cell_set_code = columns.iter().enumerate().map(|(index, column)| {
        let method_ident = Ident::new(&format!("set_{}", column.id), span);
        quote! {
            fn #method_ident<C: ::std::convert::Into<#mod_ident::Cell>>(&mut self, cell: C) {
                self.row[#index] = cell.into();
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

            fn add_empty_row(&mut self) -> #row_proxy_ident {
                let row = self.table.add_empty_row();
                #row_proxy_ident {row: row}
            }

            fn is_empty(&self) -> bool {
                self.table.is_empty()
            }

            #(#column_modify_code)*

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

        impl ::std::convert::From<#row_ident> for #mod_ident::Row {
            fn from(row: #row_ident) -> #mod_ident::Row {
                vec![#(row.#field_idents.into(),)*]
            }
        }
    }.into())
}

#[derive(FromMeta)]
struct TableParams {
    name: String,
}

impl TableParams {
    fn ident() -> Ident {
        Ident::new("table", Span::call_site())
    }

    fn parse(ast: &DeriveInput) -> GenericResult<TableParams> {
        let mut table_params = None;

        let table_ident = TableParams::ident();
        let column_ident = ColumnParams::ident();

        for attr in &ast.attrs {
            if attr.path().is_ident(&column_ident) {
                return Err!("`{}` attribute is allowed on struct fields only", column_ident);
            } else if !attr.path().is_ident(&table_ident) {
                continue;
            }

            let params = attr.parse_args_with(TableParamsParser{}).map_err(|e| format!(
                "`{}` attribute: {}", table_ident, e))?;

            if table_params.replace(params).is_some() {
                return Err!("Duplicated `{}` attribute", table_ident);
            }
        }

        Ok(table_params.unwrap_or_else(|| TableParams {
             name: "Table".to_owned(),
        }))
    }
}

struct TableParamsParser {
}

impl syn::parse::Parser for TableParamsParser {
    type Output = TableParams;

    fn parse2(self, tokens: proc_macro2::TokenStream) -> syn::Result<Self::Output> {
        Ok(TableParams::from_list(&NestedMeta::parse_meta_list(tokens)?)?)
    }
}

#[cfg_attr(test, derive(Debug))]
struct Column {
    id: String,
    name: Option<String>,
    alignment: Option<String>,
}

impl Column {
    fn parse(ast: &DeriveInput) -> GenericResult<Vec<Column>> {
        let mut columns = Vec::new();

        let table_ident = TableParams::ident();
        let column_ident = ColumnParams::ident();

        let fields = match ast.data {
            Data::Struct(DataStruct{fields: Fields::Named(ref fields), ..}) => &fields.named,
            _ => return Err!("A struct with named fields is expected"),
        };

        for field in fields {
            let field_ident = field.ident.as_ref().ok_or("A struct with named fields is expected")?;
            let mut field_params = None;

            for attr in &field.attrs {
                if attr.path().is_ident(&table_ident) {
                    return Err!("`{}` attribute is allowed on struct definition only", table_ident);
                } else if !attr.path().is_ident(&column_ident) {
                    continue;
                }

                let params = attr.parse_args_with(ColumnParamsParser{}).map_err(|e| format!(
                    "`{}` attribute on `{}` field: {}", column_ident, field_ident, e))?;

                match params.align.as_deref() {
                    Some("left") | Some("center") | Some("right") | None => {},
                    _ => return Err!("Invalid alignment of `{}`: {:?}", field_ident, params.align.unwrap()),
                };

                if field_params.replace(params).is_some() {
                    return Err!("Duplicated `{}` attribute on `{}` field", column_ident, field_ident);
                }
            }

            let column_params = field_params.unwrap_or_default();

            columns.push(Column {
                id: field_ident.to_string(),
                name: column_params.name,
                alignment: column_params.align,
            })
        }

        Ok(columns)
    }
}

#[derive(FromMeta, Default)]
struct ColumnParams {
    #[darling(default)]
    name: Option<String>,
    #[darling(default)]
    align: Option<String>,
}

impl ColumnParams {
    fn ident() -> Ident {
        Ident::new("column", Span::call_site())
    }
}

struct ColumnParamsParser {
}

impl syn::parse::Parser for ColumnParamsParser {
    type Output = ColumnParams;

    fn parse2(self, tokens: proc_macro2::TokenStream) -> syn::Result<Self::Output> {
        Ok(ColumnParams::from_list(&NestedMeta::parse_meta_list(tokens)?)?)
    }
}

