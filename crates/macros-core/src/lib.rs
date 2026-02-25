#![warn(clippy::all, clippy::pedantic)]

pub mod attr;
/// Module releted to simplex environment generation
pub mod env;
pub mod test;

/// Expands the `include_simf` macro.
///
/// # Errors
/// Returns a `syn::Result` with an error if parsing, compilation, or expansion fails.
pub fn expand_include_simf(input: &attr::parse::SynFilePath) -> syn::Result<proc_macro2::TokenStream> {
    let simf_content = attr::SimfContent::eval_path_expr(input)?;
    let abi_meta = attr::program::compile_simf(&simf_content)?;
    let generated = attr::expand_helpers(simf_content, abi_meta)?;

    Ok(generated)
}

/// Expands the `test` macro.
///
/// # Errors
/// Returns a `syn::Result` with an error if expansion fails.
pub fn expand_test(args: proc_macro2::TokenStream, input: syn::ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    test::expand(args, input)
}

pub fn expand_simplex_contract_enviroment(
    outdir: impl AsRef<std::path::Path>,
    simfs: &[impl AsRef<std::path::Path>],
) -> Result<(), env::CodeGeneratorError> {
    env::CodeGenerator::generate_files(outdir, simfs)
}
