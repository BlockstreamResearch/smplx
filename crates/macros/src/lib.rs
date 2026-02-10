use proc_macro::TokenStream;

// TODO(Illia): add path to exported crates to be able users to use their own https://stackoverflow.com/questions/79595543/rust-how-to-re-export-3rd-party-crate
//  #[serde(crate = "exporter::reexports::serde")]
//  simplicityhl, either

#[cfg(feature = "macros")]
#[proc_macro]
pub fn include_simf(tokenstream: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokenstream as simplex_macro_core::attr::parse::SynFilePath);
    match simplex_macro_core::expand_include_simf(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "macros")]
#[proc_macro_attribute]
pub fn test(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemFn);

    match simplex_macro_core::expand_test(&args.into(), &input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
