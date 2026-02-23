pub(crate) fn expand_simple(input: &syn::ItemFn) -> proc_macro2::TokenStream {
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
            use ::simplex::tracing;
            if std::env::var(simplex_test::TEST_ENV_NAME).is_err() {
                tracing::trace!("Test '{}' connected with simplex is disabled, run `simplex test` in order to test it", #fn_name_str);
                println!("disabled");
                return;
            } else {
                tracing::trace!("Running '{}' with simplex configuration", #fn_name_str);
                println!("running");
            }

            #body
            // ::sqlx::test_block_on(async { #body })
            // before
            println!("Running test: {}, \n -- {}", #fn_name_str, #ident);
            //revert
        }
    }
}
