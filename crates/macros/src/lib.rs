use proc_macro::TokenStream;

#[cfg(feature = "macros")]
#[proc_macro]
pub fn include_simf(tokenstream: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokenstream as simplex_macros_core::attr::parse::SynFilePath);

    match simplex_macros_core::expand_include_simf(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(feature = "macros")]
#[proc_macro_attribute]
pub fn test(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemFn);

    match simplex_macros_core::expand_test(args.into(), input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
