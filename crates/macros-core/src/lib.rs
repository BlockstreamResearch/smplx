#![warn(clippy::all, clippy::pedantic)]

pub mod attr;

pub(crate) mod program;

/// Expands the `include_simf` macro.
///
/// # Errors
/// Returns a `syn::Result` with an error if parsing, compilation, or expansion fails.
pub fn expand_include_simf(input: &attr::parse::SynFilePath) -> syn::Result<proc_macro2::TokenStream> {
    let simf_content = attr::SimfContent::eval_path_expr(input)?;
    let abi_meta = program::compile_simf(&simf_content)?;
    let generated = attr::expand_helpers(simf_content, abi_meta)?;

    Ok(generated)
}

/// Expands the `test` macro.
///
/// # Errors
/// Returns a `syn::Result` with an error if expansion fails.
pub fn expand_test(_args: &proc_macro2::TokenStream, input: &syn::ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    // TODO: maybe check crate attributes to allow user to do smth like in sqlx?
    Ok(expand_simple(input))
}

fn expand_simple(input: &syn::ItemFn) -> proc_macro2::TokenStream {
    let ret = &input.sig.output;
    let name = &input.sig.ident;
    let body = &input.block;
    let attrs = &input.attrs;

    let fn_name_str = name.to_string();
    let ident = format!("{input:#?}");
    quote::quote! {
        #[::core::prelude::v1::test]
        #(#attrs)*
        fn #name() #ret {
            #body
            // ::sqlx::test_block_on(async { #body })
            // before
            println!("Running test: {}, \n -- {}", #fn_name_str, #ident);
            //revert
        }
    }
}
