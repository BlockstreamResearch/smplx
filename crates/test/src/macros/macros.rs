use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parser;

use crate::TEST_ENV_NAME;

type AttributeArgs = syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>;

pub fn expand(args: TokenStream, input: syn::ItemFn) -> syn::Result<TokenStream> {
    let parser = AttributeArgs::parse_terminated;
    let args = parser.parse2(args)?;

    expand_inner(&input, args)
}

fn expand_inner(input: &syn::ItemFn, args: AttributeArgs) -> syn::Result<proc_macro2::TokenStream> {
    let ret = &input.sig.output;
    let name = &input.sig.ident;
    let inputs = &input.sig.inputs;
    let body = &input.block;
    let attrs = &input.attrs;

    let simplex_test_env = TEST_ENV_NAME;
    let ok_path_generation = {
        quote! {
            Ok(path) => {
                let path = PathBuf::from(path);
                let test_context = TestContextBuilder::FromConfigPath(path).build().unwrap();

                test_context
            }
        }
    };

    let expansion = quote::quote! {
        #[::core::prelude::v1::test]
        #(#attrs)*
        fn #name() #ret {
            use ::std::path::PathBuf;
            use ::simplex::simplex_test::TestContextBuilder;

            fn #name(#inputs) #ret {
                #body
            }

            let test_context = match std::env::var(#simplex_test_env) {
                Err(e) => {
                    panic!("Failed to run this test, required to use `simplex test`");
                }
                #ok_path_generation
            };

            #name(test_context)
        }
    };

    Ok(expansion)
}
