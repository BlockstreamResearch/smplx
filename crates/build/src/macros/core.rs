use std::error::Error;
use std::path::PathBuf;

use proc_macro2::Span;
use quote::quote;

use simplicityhl::ast::ElementsJetHinter;
use simplicityhl::{AbiMeta, TemplateProgram, UnstableFeatures};

use super::codegen::{
    GeneratedArgumentTokens, GeneratedWitnessTokens, SimfContractMeta, convert_contract_name_to_contract_module,
};
use super::parse::{SimfContent, SynFilePath};

pub fn expand(input: &SynFilePath) -> syn::Result<proc_macro2::TokenStream> {
    let simf_content = SimfContent::new(input)?;
    let abi_meta = load_abi(input, &simf_content).map_err(|e| syn::Error::new(Span::call_site(), e))?;
    let generated = expand_inner(simf_content, abi_meta).map_err(|e| syn::Error::new(Span::call_site(), e))?;

    Ok(generated)
}

/// The contract's ABI, from the sidecar `simplex build` wrote, falling back to the
/// linked frontend for a bare `include_simf!` with no sidecar.
fn load_abi(input: &SynFilePath, simf_content: &SimfContent) -> Result<AbiMeta, Box<dyn Error>> {
    if let Ok(path) = input.resolve() {
        let sidecar = PathBuf::from(format!("{}.abi.json", path.display()));
        if sidecar.is_file() {
            let json = std::fs::read_to_string(&sidecar)?;
            return crate::meta::abi_from_json(&json).map_err(Into::into);
        }
    }
    compile_simf(simf_content)
}

fn expand_inner(simf_content: SimfContent, meta: AbiMeta) -> Result<proc_macro2::TokenStream, Box<dyn Error>> {
    let mod_ident = convert_contract_name_to_contract_module(&simf_content.contract_name);

    let derived_meta = SimfContractMeta::try_from(simf_content, meta)?;

    let witness_helpers = construct_witness_helpers(&derived_meta)?;
    let arguments_helpers = construct_argument_helpers(&derived_meta)?;

    Ok(quote! {
        pub mod #mod_ident{
            #witness_helpers

            #arguments_helpers
        }
    })
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

fn compile_simf(content: &SimfContent) -> Result<AbiMeta, Box<dyn Error>> {
    // Strip a `simc` directive: only version-stable typings are extracted here, so
    // the linked frontend must not version-check it.
    let program = crate::version::without_directive(content.content.as_str());

    Ok(
        TemplateProgram::new_with_unstable(program.as_ref(), &UnstableFeatures::all(), Box::new(ElementsJetHinter))?
            .generate_abi_meta()?,
    )
}
