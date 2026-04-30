use std::error::Error;
use std::path::PathBuf;

use proc_macro2::Span;
use quote::quote;

use simplicityhl::AbiMeta;

use crate::macros::codegen::{get_contract_dependency_source_const, get_contract_source_path_name_const};

use super::codegen::{
    GeneratedArgumentTokens, GeneratedWitnessTokens, SimfContractMeta, convert_contract_name_to_contract_module,
};
use super::parse::{SimfContent, SynFilePath};
use super::program;

pub fn expand(input: &SynFilePath) -> syn::Result<proc_macro2::TokenStream> {
    let (simf_content, simf_path) = SimfContent::eval_path_expr(input)?;
    let deps_content = input.validate_and_load_deps_json()?;

    let abi_meta = program::compile_simf(&simf_path, &simf_content, &deps_content)?;
    let generated = expand_helpers(simf_path, simf_content, deps_content, abi_meta)?;

    Ok(generated)
}

fn expand_helpers(
    simf_main_file: PathBuf,
    simf_content: SimfContent,
    deps_content: String,
    meta: AbiMeta,
) -> syn::Result<proc_macro2::TokenStream> {
    gen_helpers_inner(simf_main_file, simf_content, deps_content, meta)
        .map_err(|e| syn::Error::new(Span::call_site(), e))
}

fn gen_helpers_inner(
    simf_main_file: PathBuf,
    simf_content: SimfContent,
    deps_content: String,
    meta: AbiMeta,
) -> Result<proc_macro2::TokenStream, Box<dyn Error>> {
    let mod_ident = convert_contract_name_to_contract_module(&simf_content.contract_name);

    let derived_meta = SimfContractMeta::try_from(simf_main_file, simf_content, deps_content, meta)?;

    let program_helpers = construct_program_helpers(&derived_meta);
    let witness_helpers = construct_witness_helpers(&derived_meta)?;
    let arguments_helpers = construct_argument_helpers(&derived_meta)?;

    Ok(quote! {
        pub mod #mod_ident{
            #program_helpers

            #witness_helpers

            #arguments_helpers
        }
    })
}

fn construct_program_helpers(derived_meta: &SimfContractMeta) -> proc_macro2::TokenStream {
    let contract_content = &derived_meta.simf_content.content;
    let contract_source_name = &derived_meta.contract_source_const_name;

    let contract_source_path_name = get_contract_source_path_name_const();
    let contract_source_path = derived_meta.simf_main_file.to_string_lossy();

    let contract_dependency_name = get_contract_dependency_source_const();
    let contract_dependency_content = &derived_meta.deps_content;

    quote! {
        pub const #contract_source_path_name: &str = #contract_source_path;
        pub const #contract_source_name: &str = #contract_content;
        pub const #contract_dependency_name: &str = #contract_dependency_content;
    }
}

fn construct_witness_helpers(derived_meta: &SimfContractMeta) -> syn::Result<proc_macro2::TokenStream> {
    let GeneratedWitnessTokens {
        imports,
        struct_token_stream,
        struct_impl,
    } = derived_meta.witness_struct.generate_witness_impl()?;

    Ok(quote! {
        pub use build_witness::*;
        mod build_witness {
            #imports

            #struct_token_stream

            #struct_impl
        }
    })
}

fn construct_argument_helpers(derived_meta: &SimfContractMeta) -> syn::Result<proc_macro2::TokenStream> {
    let GeneratedArgumentTokens {
        imports,
        struct_token_stream,
        struct_impl,
    } = derived_meta.args_struct.generate_arguments_impl()?;

    Ok(quote! {
        pub use build_arguments::*;
        mod build_arguments {
            #imports

            #struct_token_stream

            #struct_impl
        }
    })
}
