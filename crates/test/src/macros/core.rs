use proc_macro2::TokenStream;
use syn::parse::Parser;

use crate::TEST_ENV_NAME;

#[macro_export]
macro_rules! smplx_test_marker {
    () => {
        "_smplx_test"
    };
    (fuzz) => {
        "_smplx_fuzz"
    };
}

pub const SMPLX_TEST_MARKER: &str = smplx_test_marker!();
pub const SMPLX_FUZZ_MARKER: &str = smplx_test_marker!(fuzz);

type AttributeArgs = syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>;

pub fn expand_test(args: TokenStream, input: syn::ItemFn) -> syn::Result<TokenStream> {
    let parser = AttributeArgs::parse_terminated;
    let args = parser.parse2(args)?;

    expand_inner(&input, args)
}

pub fn expand_fuzz(args: TokenStream, input: syn::ItemFn) -> syn::Result<TokenStream> {
    let parser = AttributeArgs::parse_terminated;
    let args = parser.parse2(args)?;

    expand_fuzz_inner(&input, args)
}

// TODO: args?
fn expand_inner(input: &syn::ItemFn, _args: AttributeArgs) -> syn::Result<proc_macro2::TokenStream> {
    let ret = &input.sig.output;
    let name = quote::format_ident!("{}_{}", &input.sig.ident.to_string(), SMPLX_TEST_MARKER);
    let inputs = &input.sig.inputs;
    let body = &input.block;
    let attrs = &input.attrs;

    let simplex_test_env = TEST_ENV_NAME;
    let test_context = test_context_tokens(simplex_test_env);

    let expansion = quote::quote! {
        #[::core::prelude::v1::test]
        #(#attrs)*
        fn #name() #ret {
            fn #name(#inputs) #ret {
                #body
            }

            let test_context = #test_context;
            let test_context = test_context.regular().unwrap();

            #name(test_context)
        }
    };

    Ok(expansion)
}

// TODO: args?
fn expand_fuzz_inner(input: &syn::ItemFn, _args: AttributeArgs) -> syn::Result<proc_macro2::TokenStream> {
    let ret = &input.sig.output;
    let name = quote::format_ident!("{}_{}", &input.sig.ident.to_string(), SMPLX_FUZZ_MARKER);
    let inputs = &input.sig.inputs;
    let body = &input.block;
    let attrs = &input.attrs;

    let simplex_test_env = TEST_ENV_NAME;
    let test_context = test_context_tokens(simplex_test_env);

    let expansion = quote::quote! {
        #[::core::prelude::v1::test]
        #(#attrs)*
        fn #name() #ret {
            fn #name(#inputs) #ret {
                #body
            }

            let test_context = #test_context;

            let test_name = ::core::concat!(
                ::core::module_path!(),
                "::",
                ::core::stringify!(#name)
            );
            let source_file = ::core::concat!(
                ::core::env!("CARGO_MANIFEST_DIR"),
                "/src/",
                ::core::stringify!(#name),
                ".any"
            );
            let test_context = test_context.fuzz(test_name, source_file).unwrap();

            #name(test_context)
        }
    };

    Ok(expansion)
}

fn test_context_tokens(simplex_test_env: &str) -> TokenStream {
    quote::quote! {
        match ::std::env::var(#simplex_test_env) {
            ::core::result::Result::Err(_) => {
                ::core::panic!("Failed to run this test, required to use `simplex test`");
            },
            ::core::result::Result::Ok(path) => {
                ::simplex::TestContext::new(::std::path::PathBuf::from(path)).unwrap()
            }
        }
    }
}
