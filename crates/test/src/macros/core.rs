use crate::{NetworkUtils, TEST_ENV_NAME};

use proc_macro2::{Ident, TokenStream};
use quote::format_ident;
use syn::parse::Parser;

#[macro_export]
macro_rules! smplx_test_marker {
    () => {
        "_smplx_test"
    };
}

pub const SMPLX_TEST_MARKER: &str = smplx_test_marker!();

type AttributeArgs = syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>;

struct ParsedAttrArgs {
    mock_time: Option<MockTime>,
}

struct MockTime(u64);

pub fn expand(args: TokenStream, input: syn::ItemFn) -> syn::Result<TokenStream> {
    let parser = AttributeArgs::parse_terminated;
    let args = parser.parse2(args)?;

    expand_inner(&input, args)
}

fn expand_inner(input: &syn::ItemFn, args: AttributeArgs) -> syn::Result<TokenStream> {
    let expansion = if args.is_empty() {
        expand_simple(input)
    } else {
        expand_advanced(input, args)?
    };

    Ok(expansion)
}

/// Expands macros with zero input params - using the most simple behaviour
fn expand_simple(input: &syn::ItemFn) -> TokenStream {
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
            fn #name(#inputs) #ret {
                #body
            }

            let test_context = match ::std::env::var(#simplex_test_env) {
                ::core::result::Result::Err(_) => {
                    ::core::panic!("Failed to run this test, required to use `simplex test`");
                },
                ::core::result::Result::Ok(path) => {
                    ::simplex::TestContext::new(::std::path::PathBuf::from(path)).unwrap()
                }
            };

            #name(test_context)
        }
    };
    expansion
}

/// Expands macros with additional parameters provided in brackets.
///
/// Such as ("mock_time" and others..).
fn expand_advanced(input: &syn::ItemFn, args: AttributeArgs) -> syn::Result<TokenStream> {
    let ret = &input.sig.output;
    let name = quote::format_ident!("{}_{}", &input.sig.ident.to_string(), SMPLX_TEST_MARKER);
    let inputs = &input.sig.inputs;
    let body = &input.block;
    let attrs = &input.attrs;
    let conf_name = quote::format_ident!("conf");

    let parsed_params = ParsedAttrArgs::parse_args(args)?;
    let param_overloading = parsed_params.build_tokens(&conf_name);

    let simplex_test_env = TEST_ENV_NAME;

    let expansion = quote::quote! {
        #[::core::prelude::v1::test]
        #(#attrs)*
        fn #name() #ret {
            fn #name(#inputs) #ret {
                #body
            }

            let test_context = match ::std::env::var(#simplex_test_env) {
                ::core::result::Result::Err(_) => {
                    ::core::panic!("Failed to run this test, required to use `simplex test`");
                },
                ::core::result::Result::Ok(path) => {
                    let mut #conf_name = ::simplex::TestConfig::from_file(::std::path::PathBuf::from(path)).unwrap();
                    {
                        #param_overloading
                    }
                    ::simplex::TestContext::from_config(#conf_name).unwrap()
                }
            };

            #name(test_context)
        }
    };

    Ok(expansion)
}

impl MockTime {
    #[inline]
    fn name(&self) -> Ident {
        format_ident!("mock_time")
    }

    #[inline]
    fn value(&self) -> TokenStream {
        let value = self.0;
        quote::quote! {
            ::core::option::Option::Some(#value)
        }
    }
}

impl ParsedAttrArgs {
    const MAX_LEN: usize = 1;

    fn parse_args(attr_args: AttributeArgs) -> syn::Result<Self> {
        use syn::{Expr, Lit, Meta, MetaNameValue};

        let mut mock_time: Option<MockTime> = Default::default();

        for arg in attr_args {
            let path = arg.path().clone();

            match arg {
                // mock_time = <unsigned integer>
                Meta::NameValue(MetaNameValue { value, .. }) if path.is_ident("mock_time") => {
                    if mock_time.is_some() {
                        return Err(syn::Error::new_spanned(
                            path,
                            "cannot have more than one `mock_time` arg",
                        ));
                    }

                    let Expr::Lit(syn::ExprLit { lit: Lit::Int(lit), .. }) = value else {
                        return Err(syn::Error::new_spanned(path, "expected unsigned integer"));
                    };

                    let value = lit.base10_parse::<u64>()?;
                    NetworkUtils::validate_mock_time(value).map_err(|err| syn::Error::new_spanned(lit, err))?;

                    mock_time = Some(MockTime(value));
                }
                arg => return Err(syn::Error::new_spanned(arg, r#"expected `mock_time = 1_234_567`"#)),
            }
        }

        Ok(ParsedAttrArgs { mock_time })
    }

    fn build_tokens(&self, conf_name: &Ident) -> TokenStream {
        let mut assignments: Vec<TokenStream> = Vec::with_capacity(Self::MAX_LEN);

        if let Some(mock_time) = self.mock_time.as_ref() {
            assignments.push({
                let name = mock_time.name();
                let value = mock_time.value();

                quote::quote! {
                    #conf_name.#name = #value;
                }
            });
        }

        quote::quote! {
            #(#assignments)*
        }
    }
}
