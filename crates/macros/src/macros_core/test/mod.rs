use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::parse::Parser;

pub(crate) fn expand_inner(input: &syn::ItemFn, args: AttributeArgs) -> syn::Result<proc_macro2::TokenStream> {
    let ret = &input.sig.output;
    let name = &input.sig.ident;
    let inputs = &input.sig.inputs;
    let body = &input.block;
    let attrs = &input.attrs;

    let fn_name_str = name.to_string();
    let args_str = args.clone().to_token_stream().to_string();
    let parsed_attribute_args = parse_args(args)?;
    let simplex_test_env = simplex_test::TEST_ENV_NAME;
    let ident = format!("{input:#?}");
    let ok_path_generation = match parsed_attribute_args.config_option {
        ConfigOpt::Config => {
            quote! {
                Ok(_) => {
                    let test_context = TestContextBuilder::Default.build().unwrap();
                    tracing::trace!("Running '{}' with simplex configuration", #fn_name_str);
                    test_context
                }
            }
        }
        ConfigOpt::None => {
            quote! {
                Ok(path) => {
                    let path = PathBuf::from(path);
                    let test_context = TestContextBuilder::FromConfigPath(path).build().unwrap();
                    tracing::trace!("Running '{}' with simplex configuration", #fn_name_str);
                    test_context
                }
            }
        }
    };

    let expansion = quote::quote! {
        #[::core::prelude::v1::test]
        #(#attrs)*
        fn #name() #ret {
            use ::simplex::tracing;
            use ::std::path::PathBuf;
            use ::simplex_test::TestContextBuilder;

            fn #name(#inputs) #ret {
                #body
            }

            let test_context = match std::env::var(#simplex_test_env) {
                Err(e) => {
                    tracing::trace!(
                        "Test '{}' connected with simplex is disabled, run `simplex test` in order to test it, err: '{e}'", #fn_name_str
                    );
                    panic!("Failed to run this test, required to use `simplex test`");
                }
                #ok_path_generation
            };
            // println!("fn name: {}, \n ident: {}", #fn_name_str, #ident);
            // println!("input: {}, \n AttributeArgs: {}", "", #args_str);

            #name(test_context)
        }
    };
    Ok(expansion)
}

struct Args {
    config_option: ConfigOpt,
}

enum ConfigOpt {
    Config,
    None,
}

type AttributeArgs = syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>;

pub fn expand(args: TokenStream, input: syn::ItemFn) -> syn::Result<TokenStream> {
    let parser = AttributeArgs::parse_terminated;
    let args = parser.parse2(args)?;

    expand_inner(&input, args)
}

fn parse_args(attr_args: AttributeArgs) -> syn::Result<Args> {
    if attr_args.is_empty() {
        return Ok(Args {
            config_option: ConfigOpt::Config,
        });
    }

    if attr_args.len() > 1 {
        return Err(syn::Error::new_spanned(
            &attr_args,
            "only a single `default_rpc` flag is allowed",
        ));
    }

    match attr_args.iter().next().unwrap() {
        syn::Meta::Path(path) if path.is_ident("default_rpc") => Ok(Args {
            config_option: ConfigOpt::None,
        }),
        arg => Err(syn::Error::new_spanned(
            arg,
            "expected only the `default_rpc` flag with no assignment or value",
        )),
    }
}
