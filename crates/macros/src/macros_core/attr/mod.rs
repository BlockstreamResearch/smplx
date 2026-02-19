pub mod codegen;
pub mod parse;
mod types;

pub use parse::SimfContent;

use crate::macros_core::attr::codegen::{GeneratedArgumentTokens, GeneratedWitnessTokens, SimfContractMeta};
use proc_macro2::Span;
use quote::{format_ident, quote};
use simplicityhl::AbiMeta;
use std::error::Error;
// TODO(Illia): add bincode generation feature (i.e. require bincode dependencies)
// TODO(Illia): add conditional compilation for simplicity-core to e included automatically

// TODO(Illia): automatically derive bincode implementation
// TODO(Illia): extract either:serde feature and use it when simplicityhl has serde feature
// TODO(Illia): add features

/// Expands helper functions for the given Simf content and metadata.
///
/// # Errors
/// Returns a `syn::Result` with an error if code generation fails.
pub fn expand_helpers(simf_content: SimfContent, meta: AbiMeta) -> syn::Result<proc_macro2::TokenStream> {
    gen_helpers_inner(simf_content, meta).map_err(|e| syn::Error::new(Span::call_site(), e))
}

fn gen_helpers_inner(simf_content: SimfContent, meta: AbiMeta) -> Result<proc_macro2::TokenStream, Box<dyn Error>> {
    let mod_ident = format_ident!("derived_{}", simf_content.contract_name);

    let derived_meta = SimfContractMeta::try_from(simf_content, meta)?;

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
    let error_msg = format!(
        "INTERNAL: expected '{}' Program to compile successfully.",
        derived_meta.simf_content.contract_name
    );
    let contract_source_name = &derived_meta.contract_source_const_name;
    let contract_arguments_struct_name = &derived_meta.args_struct.struct_name;

    quote! {
        // use simplicityhl::elements::Address;
        // use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;
        // use simplex::simplex_core::{create_p2tr_address, load_program, ProgramError, SimplicityNetwork};
        // use simplicityhl::CompiledProgram;

        // pub const #contract_source_name: &str = #contract_content;

        /// Get the options template program for instantiation.
        ///
        /// # Panics
        /// - if the embedded source fails to compile (should never happen).
        // #[must_use]
        // pub fn get_template_program() -> ::simplicityhl::TemplateProgram {
        //     ::simplicityhl::TemplateProgram::new(#contract_source_name).expect(#error_msg)
        // }

        /// Derive P2TR address for an option offer contract.
        ///
        /// # Errors
        ///
        /// Returns error if program compilation fails.
        // pub fn get_option_offer_address(
        //     x_only_public_key: &XOnlyPublicKey,
        //     arguments: &#contract_arguments_struct_name,
        //     network: SimplicityNetwork,
        // ) -> Result<Address, ProgramError> {
        //     Ok(create_p2tr_address(
        //         get_loaded_program(arguments)?.commit().cmr(),
        //         x_only_public_key,
        //         network.address_params(),
        //     ))
        // }

        /// Compile option offer program with the given arguments.
        ///
        /// # Errors
        ///
        /// Returns error if compilation fails.
        // pub fn get_loaded_program(
        //     arguments: &#contract_arguments_struct_name,
        // ) -> Result<CompiledProgram, ProgramError> {
        //     load_program(#contract_source_name, arguments.build_arguments())
        // }

        /// Get compiled option offer program, panicking on failure.
        ///
        /// # Panics
        ///
        /// Panics if program instantiation fails.
        // #[must_use]
        // pub fn get_compiled_program(arguments: &#contract_arguments_struct_name) -> CompiledProgram {
        //     let program = get_template_program();

        //     program
        //         .instantiate(arguments.build_arguments(), true)
        //         .unwrap()
        // }
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
