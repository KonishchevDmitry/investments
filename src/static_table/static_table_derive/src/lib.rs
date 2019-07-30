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

    let row_struct = match ast.data {
        Data::Struct(ref row_struct) => row_struct,
        _ => return Err!("A struct is expected"),
    };

    // FIXME: HERE
    let table_name = get_table_params(&ast)?;

    // Build the trait implementation
    Ok(impl_static_table(&ast))
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
            return Err!("Duplicated {:?} attribute", TABLE_ATTR_NAME)
        }
    }

    Ok(table_name.unwrap_or_else(|| String::from("Table")))
}

fn get_attribute_ident(meta: &Meta) -> &Ident {
    match meta {
        Meta::Word(ident) => ident,
        Meta::List(MetaList{ident, ..}) => ident,
        Meta::NameValue(MetaNameValue{ident, ..}) => ident,
    }
}

// FIXME: HERE
fn impl_static_table(ast: &DeriveInput) -> TokenStream {
    match ast.data {
        // If the attribute was applied to a struct, we're going to do
        // some more work to figure out if there's a field named "bees".
        // It's important to take a reference to `struct_item`, otherwise
        // you partially move `item`.
        Data::Struct(ref struct_item) => {
            match struct_item.fields {
                // A field can only be named "bees" if it has a name, so we'll
                // match those fields and ignore the rest.
                Fields::Named(ref fields) => {
                    for field in &fields.named {
                        println!(">>> {:?} {:?}", field.ident, field.attrs);
                        for attr in &field.attrs {
                            println!("> {:?}", attr.interpret_meta());
//                            println!("Z> {:?}", MacroArgs::from_meta(&attr.interpret_meta().unwrap()).unwrap());
//                            let args: TokenStream = attr.tts.clone().into();

//                            let args: TokenStream = attr.into_token_stream().into();
//                            let attr_args = parse_macro_input!(args as AttributeArgs);
//                            println!("Z> {:?}", MacroArgs::from_list(&attr_args).unwrap());
                        }
                    }
                }
                // Ignore unit structs or anonymous fields.
                _ => {
                }
            }
        },

        // If the attribute was applied to any other kind of item, we want
        // to generate a compiler error.
        _ => {
            // This is how you generate a compiler error. You can also
            // generate a "note," or a "warning."
            panic!("This is not a struct");
//            ast.span().unstable()
//                .error()
//                .emit();
        },
    }

    let name = &ast.ident;
    let gen = quote! {
        impl HelloMacro for #name {
            fn hello_macro() {
                println!("Hello, Macro! My name is {}", stringify!(#name));
            }
        }
    };
    gen.into()
}