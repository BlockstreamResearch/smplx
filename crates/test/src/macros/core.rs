use proc_macro2::TokenStream;
use syn::parse::{Parse, ParseStream};

use crate::TEST_ENV_NAME;

pub const SMPLX_TEST_MARKER: &str = "_smplx_test";

pub fn expand(args: TokenStream, input: syn::ItemFn) -> syn::Result<TokenStream> {
    let args = syn::parse2(args)?;
    expand_inner(&input, args)
}

fn expand_inner(input: &syn::ItemFn, args: TestArgs) -> syn::Result<TokenStream> {
    let log_level_init = args.log_level_init();

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

            #log_level_init

            let test_context = match std::env::var(#simplex_test_env) {
                Err(_) => {
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

struct TestArgs {
    log_level: Option<syn::Ident>,
}

impl Parse for TestArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self { log_level: None });
        }

        let key: syn::Ident = input.parse()?;
        let _eq: syn::Token![=] = input.parse()?;

        match key.to_string().as_str() {
            "log_level" => Ok(Self {
                log_level: Some(input.parse()?),
            }),
            other => Err(syn::Error::new(key.span(), format!("unknown argument `{other}`"))),
        }
    }
}

impl TestArgs {
    fn log_level_init(&self) -> Option<TokenStream> {
        self.log_level.as_ref().map(|level| {
            quote::quote! {
                ::simplex::set_tracker_log_level(::simplex::TrackerLogLevel::#level);
            }
        })
    }
}
