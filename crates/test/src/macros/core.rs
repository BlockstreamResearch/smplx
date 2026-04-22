use proc_macro2::TokenStream;
use syn::parse::Parser;

use crate::TEST_ENV_NAME;

pub const SMPLX_TEST_MARKER: &str = "_smplx_test";

type AttributeArgs = syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>;

pub fn expand(args: TokenStream, input: syn::ItemFn) -> syn::Result<TokenStream> {
    let parser = AttributeArgs::parse_terminated;
    let args = parser.parse2(args)?;

    expand_inner(&input, args)
}

// TODO: args?
fn expand_inner(input: &syn::ItemFn, _args: AttributeArgs) -> syn::Result<proc_macro2::TokenStream> {
    let ret = &input.sig.output;
    let name = quote::format_ident!("{}_{}", &input.sig.ident.to_string(), SMPLX_TEST_MARKER);
    let inputs = &input.sig.inputs;
    let body = &input.block;
    let attrs = &input.attrs;

    let simplex_test_env = TEST_ENV_NAME;

    let expansion = quote::quote! {
        #[::core::prelude::v1::test]
        #(#attrs)*
        fn #name() #ret {
            use std::path::PathBuf;
            use simplex::TestContext;

            fn #name(#inputs) #ret {
                #body
            }

            let test_context = match std::env::var(#simplex_test_env) {
                Err(e) => {
                    panic!("Failed to run this test, required to use `simplex test`");
                },
                Ok(path) => {
                    TestContext::new(PathBuf::from(path)).unwrap()
                }
            };

            #name(test_context)
        }
    };

    Ok(expansion)
}
