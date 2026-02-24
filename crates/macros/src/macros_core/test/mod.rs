use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::parse::Parser;

pub(crate) fn expand_simple(input: &syn::ItemFn, args: AttributeArgs) -> proc_macro2::TokenStream {
    let ret = &input.sig.output;
    let name = &input.sig.ident;
    let inputs = &input.sig.inputs;
    let body = &input.block;
    let attrs = &input.attrs;

    let fn_name_str = name.to_string();
    let args = args.to_token_stream().to_string();
    let simplex_test_env = simplex_test::TEST_ENV_NAME;
    let ident = format!("{input:#?}");
    quote::quote! {
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
                Ok(path) => {
                    let path = PathBuf::from(path);
                    let test_context = TestContextBuilder::FromConfigPath(path).build().unwrap();
                    tracing::trace!("Running '{}' with simplex configuration", #fn_name_str);
                    test_context
                }
            };
            // before
            println!("fn name: {}, \n ident: {}", #fn_name_str, #ident);
            println!("input: {}, \n AttributeArgs: {}", "", #args);

            #name(test_context)
        }
    }
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

    Ok(expand_simple(&input, args))

    // if input.sig.inputs.is_empty() {
    //     if !args.is_empty() {
    //         if cfg!(not(feature = "migrate")) {
    //             return Err(syn::Error::new_spanned(
    //                 args.first().unwrap(),
    //                 "control attributes are not allowed unless \
    //                     the `migrate` feature is enabled and \
    //                     automatic test DB management is used; see docs",
    //             )
    //             .into());
    //         }
    //
    //         return Err(syn::Error::new_spanned(
    //             args.first().unwrap(),
    //             "control attributes are not allowed unless \
    //                 automatic test DB management is used; see docs",
    //         )
    //         .into());
    //     }
    //
    //     return Ok(expand_simple(&input, args));
    // }
    //
    // expand_advanced(args, input)
}

// fn expand_advanced(args: AttributeArgs, input: syn::ItemFn) -> syn::Result<TokenStream> {
//     let config = sqlx_core::config::Config::try_from_crate_or_default()?;
//
//     let ret = &input.sig.output;
//     let name = &input.sig.ident;
//     let inputs = &input.sig.inputs;
//     let body = &input.block;
//     let attrs = &input.attrs;
//
//     let args = parse_args(args)?;
//
//     let fn_arg_types = inputs.iter().map(|_| quote! { _ });
//
//     let mut fixtures = Vec::new();
//
//     for (fixture_type, fixtures_local) in args.fixtures {
//         let mut res = match fixture_type {
//             FixturesType::None => vec![],
//             FixturesType::RelativePath => fixtures_local
//                 .into_iter()
//                 .map(|fixture| {
//                     let mut fixture_str = fixture.value();
//                     add_sql_extension_if_missing(&mut fixture_str);
//
//                     let path = format!("fixtures/{}", fixture_str);
//
//                     quote! {
//                         ::sqlx::testing::TestFixture {
//                             path: #path,
//                             contents: include_str!(#path),
//                         }
//                     }
//                 })
//                 .collect(),
//             FixturesType::CustomRelativePath(path) => fixtures_local
//                 .into_iter()
//                 .map(|fixture| {
//                     let mut fixture_str = fixture.value();
//                     add_sql_extension_if_missing(&mut fixture_str);
//
//                     let path = format!("{}/{}", path.value(), fixture_str);
//
//                     quote! {
//                         ::sqlx::testing::TestFixture {
//                             path: #path,
//                             contents: include_str!(#path),
//                         }
//                     }
//                 })
//                 .collect(),
//             FixturesType::ExplicitPath => fixtures_local
//                 .into_iter()
//                 .map(|fixture| {
//                     let path = fixture.value();
//
//                     quote! {
//                         ::sqlx::testing::TestFixture {
//                             path: #path,
//                             contents: include_str!(#path),
//                         }
//                     }
//                 })
//                 .collect(),
//         };
//         fixtures.append(&mut res)
//     }
//
//     let migrations = match args.migrations {
//         ConfigOpt::ExplicitPath(path) => {
//             let migrator = crate::migrate::expand(Some(path))?;
//             quote! { args.migrator(&#migrator); }
//         }
//         ConfigOpt::InferredPath if !inputs.is_empty() => {
//             let path = crate::migrate::default_path(&config);
//
//             let resolved_path = crate::common::resolve_path(path, proc_macro2::Span::call_site())?;
//
//             if resolved_path.is_dir() {
//                 let migrator = crate::migrate::expand_with_path(&config, &resolved_path)?;
//                 quote! { args.migrator(&#migrator); }
//             } else {
//                 quote! {}
//             }
//         }
//         ConfigOpt::ExplicitMigrator(path) => {
//             quote! { args.migrator(&#path); }
//         }
//         _ => quote! {},
//     };
//
//     Ok(quote! {
//         #(#attrs)*
//         #[::core::prelude::v1::test]
//         fn #name() #ret {
//             async fn #name(#inputs) #ret {
//                 #body
//             }
//
//             let mut args = ::sqlx::testing::TestArgs::new(concat!(module_path!(), "::", stringify!(#name)));
//
//             #migrations
//
//             args.fixtures(&[#(#fixtures),*]);
//
//             // We need to give a coercion site or else we get "unimplemented trait" errors.
//             let f: fn(#(#fn_arg_types),*) -> _ = #name;
//
//             ::sqlx::testing::TestFn::run_test(f, args)
//         }
//     })
// }

// fn parse_args(attr_args: AttributeArgs) -> syn::Result<Args> {
//     use syn::{
//         parenthesized, parse::Parse, punctuated::Punctuated, token::Comma, Expr, Lit, LitStr, Meta, MetaNameValue,
//         Token,
//     };
//
//     let mut fixtures = Vec::new();
//     let mut migrations = ConfigOpt::InferredPath;
//
//     for arg in attr_args {
//         let path = arg.path().clone();
//
//         match arg {
//             syn::Meta::List(list) if list.path.is_ident("fixtures") => {
//                 let mut fixtures_local = vec![];
//                 let mut fixtures_type = FixturesType::None;
//
//                 let parse_nested = list.parse_nested_meta(|meta| {
//                     if meta.path.is_ident("path") {
//                         //  fixtures(path = "<path>", scripts("<file_1>","<file_2>")) checking `path` argument
//                         meta.input.parse::<Token![=]>()?;
//                         let val: LitStr = meta.input.parse()?;
//                         parse_fixtures_path_args(&mut fixtures_type, val)?;
//                     } else if meta.path.is_ident("scripts") {
//                         //  fixtures(path = "<path>", scripts("<file_1>","<file_2>")) checking `scripts` argument
//                         let content;
//                         parenthesized!(content in meta.input);
//                         let list = content.parse_terminated(<LitStr as Parse>::parse, Comma)?;
//                         parse_fixtures_scripts_args(&mut fixtures_type, list, &mut fixtures_local)?;
//                     } else {
//                         return Err(syn::Error::new_spanned(meta.path, "unexpected fixture meta"));
//                     }
//
//                     Ok(())
//                 });
//
//                 if parse_nested.is_err() {
//                     // fixtures("<file_1>","<file_2>") or fixtures("<path/file_1.sql>","<path/file_2.sql>")
//                     let args = list.parse_args_with(<Punctuated<LitStr, Token![,]>>::parse_terminated)?;
//                     for arg in args {
//                         parse_fixtures_args(&mut fixtures_type, arg, &mut fixtures_local)?;
//                     }
//                 }
//
//                 fixtures.push((fixtures_type, fixtures_local));
//             }
//             syn::Meta::NameValue(value) if value.path.is_ident("migrations") => {
//                 if !matches!(migrations, ConfigOpt::InferredPath) {
//                     return Err(syn::Error::new_spanned(
//                         value,
//                         "cannot have more than one `migrations` or `migrator` arg",
//                     ));
//                 }
//
//                 fn recurse_lit_lookup(expr: Expr) -> Option<Lit> {
//                     match expr {
//                         Expr::Lit(syn::ExprLit { lit, .. }) => Some(lit),
//                         Expr::Group(syn::ExprGroup { expr, .. }) => recurse_lit_lookup(*expr),
//                         _ => None,
//                     }
//                 }
//
//                 let Some(lit) = recurse_lit_lookup(value.value) else {
//                     return Err(syn::Error::new_spanned(path, "expected string or `false`"));
//                 };
//
//                 migrations = match lit {
//                     // migrations = false
//                     Lit::Bool(b) if !b.value => ConfigOpt::Disabled,
//                     // migrations = true
//                     Lit::Bool(b) => {
//                         return Err(syn::Error::new_spanned(b, "`migrations = true` is redundant"));
//                     }
//                     // migrations = "path"
//                     Lit::Str(s) => ConfigOpt::ExplicitPath(s),
//                     lit => return Err(syn::Error::new_spanned(lit, "expected string or `false`")),
//                 };
//             }
//             // migrator = "<path>"
//             Meta::NameValue(MetaNameValue { value, .. }) if path.is_ident("migrator") => {
//                 if !matches!(migrations, ConfigOpt::InferredPath) {
//                     return Err(syn::Error::new_spanned(
//                         path,
//                         "cannot have more than one `migrations` or `migrator` arg",
//                     ));
//                 }
//
//                 let Expr::Lit(syn::ExprLit { lit: Lit::Str(lit), .. }) = value else {
//                     return Err(syn::Error::new_spanned(path, "expected string"));
//                 };
//
//                 migrations = ConfigOpt::ExplicitMigrator(lit.parse()?);
//             }
//             arg => {
//                 return Err(syn::Error::new_spanned(
//                     arg,
//                     r#"expected `fixtures("<filename>", ...)` or `migrations = "<path>" | false` or `migrator = "<rust path>"`"#,
//                 ));
//             }
//         }
//     }
//
//     Ok(Args { config_option: ConfigOpt::None })
// }
