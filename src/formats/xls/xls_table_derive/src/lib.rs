use darling::FromMeta;
use darling::ast::NestedMeta;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{self, Data, DataStruct, DeriveInput, Expr, ExprArray, Fields, Lit};

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type GenericResult<T> = Result<T, GenericError>;

macro_rules! Err {
    ($($arg:tt)*) => (::std::result::Result::Err(format!($($arg)*).into()))
}

#[proc_macro_derive(HtmlTableRow, attributes(table, column))]
pub fn html_table_row_derive(input: TokenStream) -> TokenStream {
    match table_row_derive_impl(input, false) {
        Ok(output) => output,
        Err(err) => panic!("{}", err),
    }
}

#[proc_macro_derive(XlsTableRow, attributes(table, column))]
pub fn xls_table_row_derive(input: TokenStream) -> TokenStream {
    match table_row_derive_impl(input, true) {
        Ok(output) => output,
        Err(err) => panic!("{}", err),
    }
}

fn table_row_derive_impl(input: TokenStream, strict_parsing: bool) -> GenericResult<TokenStream> {
    let ast: DeriveInput = syn::parse(input)?;
    let span = Span::call_site();

    let table = TableParams::parse(&ast)?;
    let trim_title_func = match table.trim_column_title_with {
        Some(name) => {
            let ident = Ident::new(&name, span);
            quote!(#ident)
        },
        None => quote!(::std::borrow::Cow::from),
    };

    let columns = Column::parse(&ast)?;
    let mod_ident = quote!(crate::formats::xls);
    let row_ident = &ast.ident;

    let columns_code = columns.iter().map(|column| {
        let name = &column.name;
        let regex = column.regex;
        let case_insensitive = table.case_insensitive_match;
        let space_insensitive = table.space_insensitive_match;
        let optional = column.optional;
        let aliases = column.aliases.iter().map(|alias| quote!(#alias));
        quote!(#mod_ident::TableColumn::new(#name, #regex, &[#(#aliases,)*], #case_insensitive, #space_insensitive, #optional))
    });

    let columns_parse_code = columns.iter().enumerate().map(|(id, column)| {
        let field = Ident::new(&column.field, span);
        let name = &column.name;
        let strict = column.strict.unwrap_or(strict_parsing);

        let mut parse_code = match column.parse_with {
            Some(ref parse_func) => {
                let parse_func = parse_func.parse::<proc_macro2::TokenStream>().unwrap();
                quote!(#mod_ident::parse_with(cell, #parse_func))
            },
            None => quote!(#mod_ident::CellType::parse(cell, #strict)),
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

            fn trim_column_title(title: &str) -> ::std::borrow::Cow<str> {
                #trim_title_func(title)
            }

            fn parse(row: &[Option<&#mod_ident::Cell>]) -> crate::core::GenericResult<#row_ident> {
                Ok(#row_ident {
                    #(#columns_parse_code,)*
                })
            }
        }
    }.into())
}

#[derive(Default, FromMeta)]
struct TableParams {
    #[darling(default)]
    case_insensitive_match: bool,

    #[darling(default)]
    space_insensitive_match: bool,

    #[darling(default)]
    trim_column_title_with: Option<String>,
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

        Ok(table_params.unwrap_or_default())
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

struct Column {
    field: String,
    name: String,
    regex: bool,
    aliases: Vec<String>,
    strict: Option<bool>,
    parse_with: Option<String>,
    optional: bool,
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

                if field_params.replace(params).is_some() {
                    return Err!("Duplicated `{}` attribute on `{}` field", column_ident, field_ident);
                }
            }

            let column_params = field_params.ok_or_else(|| format!(
                "Column name is not specified for `{}` field", field_ident
            ))?;

            let mut aliases = column_params.aliases;
            if let Some(alias) = column_params.alias {
                aliases.push(alias);
            }

            columns.push(Column {
                field: field_ident.to_string(),
                name: column_params.name,
                regex: column_params.regex,
                aliases: aliases,
                strict: column_params.strict,
                parse_with: column_params.parse_with,
                optional: column_params.optional,
            })
        }

        Ok(columns)
    }
}

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
    strict: Option<bool>,
    #[darling(default)]
    parse_with: Option<String>,
    #[darling(default)]
    optional: bool,
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