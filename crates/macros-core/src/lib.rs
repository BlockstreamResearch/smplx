pub mod attr;
pub mod test;

pub fn expand_include_simf(input: &attr::parse::SynFilePath) -> syn::Result<proc_macro2::TokenStream> {
    let simf_content = attr::SimfContent::eval_path_expr(input)?;
    let abi_meta = attr::program::compile_simf(&simf_content)?;
    let generated = attr::expand_helpers(simf_content, abi_meta)?;

    Ok(generated)
}

pub fn expand_test(args: proc_macro2::TokenStream, input: syn::ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    test::expand(args, input)
}
