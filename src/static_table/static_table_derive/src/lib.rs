// FIXME

extern crate proc_macro;

use crate::proc_macro::TokenStream;
use quote::quote;
use syn::{self, parse_macro_input, Item, Fields, Data};



use darling::FromMeta;
use syn::{AttributeArgs, ItemFn};
use quote::ToTokens;

#[derive(Debug, FromMeta)]
struct MacroArgs {
//    timeout_ms: Option<u16>,
    name: String,
    description: String,
    #[darling(default)]
    skip: bool,
}


#[proc_macro_derive(StaticTable, attributes(cell, table))]
pub fn static_table_derive(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_static_table(&ast)
}

fn impl_static_table(ast: &syn::DeriveInput) -> TokenStream {
    println!("!!!! {:?}", ast.attrs);

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
                            println!("Z> {:?}", MacroArgs::from_meta(&attr.interpret_meta().unwrap()).unwrap());
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