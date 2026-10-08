use std::error::Error;

use proc_macro2::{Span, TokenStream};
use quote::quote;

use simplicityhl::ast::ElementsJetHinter;
use simplicityhl::error::DiagnosticManager;
use simplicityhl::{AbiMeta, TemplateAst, UnstableFeatures, Value};

use super::codegen::{
    GeneratedArgumentTokens, GeneratedProgramTraitHelperTokens, GeneratedWitnessTokens, SimfContractMeta,
    convert_contract_name_to_contract_module,
};
use super::parse::{SimfContent, SynFilePath};

use crate::macros::types::Constants;

pub fn expand(input: &SynFilePath) -> syn::Result<TokenStream> {
    let simf_content = SimfContent::new(input)?;

    let (abi_meta, constants) = compile_simf(&simf_content).map_err(|e| syn::Error::new(Span::call_site(), e))?;

    let generated =
        expand_inner(simf_content, abi_meta, constants).map_err(|e| syn::Error::new(Span::call_site(), e))?;

    Ok(generated)
}

fn expand_inner(
    simf_content: SimfContent,
    meta: AbiMeta,
    constants: Vec<Value>,
) -> Result<TokenStream, Box<dyn Error>> {
    let mod_ident = convert_contract_name_to_contract_module(&simf_content.contract_name);

    let derived_meta = SimfContractMeta::try_from(simf_content, meta, constants)?;

    let program_helpers = construct_program_helpers(&derived_meta);
    let witness_helpers = construct_witness_helpers(&derived_meta)?;
    let arguments_helpers = construct_argument_helpers(&derived_meta)?;
    let program_trait_helpers = construct_program_trait_helpers(&derived_meta)?;
    let constants_helpers = construct_constant_helpers(&derived_meta)?;

    Ok(quote! {
        pub mod #mod_ident{
            #program_helpers

            #witness_helpers

            #arguments_helpers

            #program_trait_helpers

            #constants_helpers
        }
    })
}

fn construct_program_helpers(derived_meta: &SimfContractMeta) -> TokenStream {
    let contract_content = &derived_meta.simf_content.content;
    let contract_source_name = &derived_meta.contract_source_const_name;

    quote! {
        pub const #contract_source_name: &str = #contract_content;
    }
}

fn construct_witness_helpers(derived_meta: &SimfContractMeta) -> syn::Result<TokenStream> {
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

fn construct_argument_helpers(derived_meta: &SimfContractMeta) -> syn::Result<TokenStream> {
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

fn compile_simf(content: &SimfContent) -> Result<(AbiMeta, Vec<Value>), Box<dyn Error>> {
    let program = content.content.as_str();

    let template = TemplateAst::new_with_unstable(program, &UnstableFeatures::all(), Box::new(ElementsJetHinter))?;

    let mut diagnostics = DiagnosticManager::default();
    let Some(analyzed) = simplicityhl::ast::Program::analyze(
        template.resolved_program(),
        Box::new(ElementsJetHinter),
        &mut diagnostics,
    ) else {
        return Err(Box::new(diagnostics));
    };

    let constants = Constants::collect(&analyzed).into_iter().cloned().collect();
    let abi_meta = template.generate_abi_meta()?;

    Ok((abi_meta, constants))
}

fn construct_program_trait_helpers(derived_meta: &SimfContractMeta) -> syn::Result<TokenStream> {
    let GeneratedProgramTraitHelperTokens { imports, helper_impls } =
        derived_meta.generate_program_trait_helpers_impl()?;

    Ok(quote! {
        mod program_helpers {
            #imports

            #helper_impls
        }
    })
}

fn construct_constant_helpers(derived_meta: &SimfContractMeta) -> syn::Result<TokenStream> {
    let GeneratedProgramTraitHelperTokens { imports, helper_impls } = derived_meta.generate_constants()?;

    Ok(quote! {
        mod build_constants {
            #imports

            #helper_impls
        }
    })
}
