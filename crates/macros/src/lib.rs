use proc_macro::TokenStream;

#[proc_macro]
pub fn include_simf(tokenstream: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokenstream as smplx_build_internal::macros::parse::SynFilePath);

    match smplx_build_internal::macros::expand(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn test(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::ItemFn);

    match smplx_test::macros::expand(args.into(), input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
