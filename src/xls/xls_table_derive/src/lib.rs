extern crate proc_macro;

use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{self, Data, DataStruct, DeriveInput, Expr, ExprArray, Fields, Lit, Meta, MetaList, MetaNameValue};

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type GenericResult<T> = Result<T, GenericError>;

struct Column {
    field: String,
    name: String,
    regex: bool,
    aliases: Vec<String>,
    parse_with: Option<String>,
    optional: bool,
}

macro_rules! Err {
    ($($arg:tt)*) => (::std::result::Result::Err(format!($($arg)*).into()))
}

#[proc_macro_derive(XlsTableRow, attributes(column))]
pub fn xls_table_row_derive(input: TokenStream) -> TokenStream {
    match xls_table_row_derive_impl(input) {
        Ok(output) => output,
        Err(err) => panic!("{}", err),
    }
}

fn xls_table_row_derive_impl(input: TokenStream) -> GenericResult<TokenStream> {
    let ast: DeriveInput = syn::parse(input)?;
    let span = Span::call_site();

    let columns = get_table_columns(&ast)?;
    let mod_ident = quote!(crate::xls);
    let row_ident = &ast.ident;

    let columns_code = columns.iter().map(|column| {
        let name = &column.name;
        let regex = column.regex;
        let optional = column.optional;
        let aliases = column.aliases.iter().map(|alias| quote!(#alias));
        quote!(#mod_ident::TableColumn::new(#name, #regex, &[#(#aliases,)*], #optional))
    });

    let columns_parse_code = columns.iter().enumerate().map(|(id, column)| {
        let field = Ident::new(&column.field, span);
        let name = &column.name;

        let mut parse_code = match column.parse_with {
            Some(ref parse_func) => {
                let parse_func = Ident::new(&parse_func, span);
                quote!(#mod_ident::parse_with(cell, #parse_func))
            },
            None => quote!(#mod_ident::CellType::parse(cell)),
        };
        parse_code = quote! {
            #parse_code.map_err(|e| format!("Column {:?}: {}", #name, e))?
        };

        let parser_code = if column.optional {
            quote! {
                match row[#id] {
                    Some(cell) => #parse_code,
                    None => None,
                }
            }
        } else {
            quote! {
                {
                    let cell = row[#id].unwrap();
                    #parse_code
                }
            }
        };

        quote! {
            #field: #parser_code
        }
    });

    Ok(quote! {
        impl #mod_ident::TableRow for #row_ident {
            fn columns() -> Vec<#mod_ident::TableColumn> {
                vec![#(#columns_code,)*]
            }

            fn parse(row: &[Option<&#mod_ident::Cell>]) -> crate::core::GenericResult<#row_ident> {
                Ok(#row_ident {
                    #(#columns_parse_code,)*
                })
            }
        }
    }.into())
}

fn get_table_columns(ast: &DeriveInput) -> GenericResult<Vec<Column>> {
    #[derive(FromMeta)]
    struct ColumnParams {
        name: String,
        #[darling(default)]
        regex: bool,
        #[darling(default)]
        alias: Option<String>,
        #[darling(default, map="parse_string_array")]
        aliases: Vec<String>,
        #[darling(default)]
        parse_with: Option<String>,
        #[darling(default)]
        optional: bool,
    }
    let column_attr_name = "column";

    let mut columns = Vec::new();

    let fields = match ast.data {
        Data::Struct(DataStruct{fields: Fields::Named(ref fields), ..}) => &fields.named,
        _ => return Err!("A struct with named fields is expected"),
    };

    for field in fields {
        let field_name = field.ident.as_ref()
            .ok_or("A struct with named fields is expected")?.to_string();
        let mut field_params = None;

        for attr in &field.attrs {
            let meta = attr.parse_meta().map_err(|e| format!(
                "Failed to parse `{:#?}` on {:?} field: {}", attr, field_name, e))?;

            if !match_attribute_name(&meta, column_attr_name) {
                continue;
            }

            let params = ColumnParams::from_meta(&meta).map_err(|e| format!(
                "{:?} attribute on {:?} field validation error: {}",
                column_attr_name, field_name, e))?;

            if field_params.replace(params).is_some() {
                return Err!("Duplicated {:?} attribute on {:?} field", column_attr_name, field_name);
            }
        }

        let column_params = field_params.ok_or_else(|| format!(
            "Column name is not specified for {:?} field", field_name
        ))?;

        let mut aliases = column_params.aliases;
        if let Some(alias) = column_params.alias {
            aliases.push(alias);
        }

        columns.push(Column {
            field: field_name,
            name: column_params.name,
            regex: column_params.regex,
            aliases: aliases,
            parse_with: column_params.parse_with,
            optional: column_params.optional,
        })
    }

    Ok(columns)
}

// Please note that due to Darling limitations the array must be specified as `array = r#"["a", "b"]"#`
fn parse_string_array(value: Lit) -> Vec<String> {
    let expr_array = ExprArray::from_value(&value).map_err(|e| format!(
        "Unexpected literal where string array is expected: {}", e)).unwrap();

    let mut array = Vec::new();

    for expr in expr_array.elems.iter() {
        let item = match expr {
            Expr::Lit(lit) => String::from_value(&lit.lit).map_err(|e| format!(
                "Unexpected literal where string array item is expected: {}", e)).unwrap(),
            _ => panic!("Unexpected expression where string array item is expected"),
        };
        array.push(item);
    }

    array
}

fn match_attribute_name(meta: &Meta, name: &str) -> bool {
    let path = match meta {
        Meta::Path(path) => path,
        Meta::List(MetaList{path, ..}) => path,
        Meta::NameValue(MetaNameValue{path, ..}) => path,
    };

    path.segments.len() == 1 && path.segments.first().unwrap().ident == name
}