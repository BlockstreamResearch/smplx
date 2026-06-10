use quote::{format_ident, quote};

use simplicityhl::str::WitnessName;
use simplicityhl::{AbiMeta, Parameters, ResolvedType, WitnessTypes};

use crate::macros::parse::SimfContent;
use crate::macros::types::RustType;

pub struct SimfContractMeta {
    pub contract_source_const_name: proc_macro2::Ident,
    pub program_struct_name: proc_macro2::Ident,
    pub args_struct: WitnessStruct,
    pub witness_struct: WitnessStruct,
    pub simf_content: SimfContent,
    pub abi_meta: AbiMeta,
}

pub struct GeneratedArgumentTokens {
    pub imports: proc_macro2::TokenStream,
    pub struct_token_stream: proc_macro2::TokenStream,
    pub struct_impl: proc_macro2::TokenStream,
}

pub struct GeneratedWitnessTokens {
    pub imports: proc_macro2::TokenStream,
    pub struct_token_stream: proc_macro2::TokenStream,
    pub struct_impl: proc_macro2::TokenStream,
}

pub struct GeneratedMutanTestingTokens {
    pub imports: proc_macro2::TokenStream,
    pub helper_impls: proc_macro2::TokenStream,
}

pub struct WitnessField {
    witness_simf_name: String,
    struct_rust_field: proc_macro2::Ident,
    rust_type: RustType,
}

pub struct WitnessStruct {
    pub struct_name: proc_macro2::Ident,
    pub witness_values: Vec<WitnessField>,
}

impl SimfContractMeta {
    /// Try to create a new `SimfContractMeta` from `SimfContent` and `AbiMeta`.
    ///
    /// # Errors
    /// Returns a `syn::Result` with an error if the arguments or witness structure cannot be generated.
    pub fn try_from(simf_content: SimfContent, abi_meta: AbiMeta) -> syn::Result<Self> {
        let args_struct = WitnessStruct::generate_args_struct(&simf_content.contract_name, &abi_meta.param_types)?;
        let witness_struct =
            WitnessStruct::generate_witness_struct(&simf_content.contract_name, &abi_meta.witness_types)?;
        let contract_source_const_name = convert_contract_name_to_contract_source_const(&simf_content.contract_name);
        let program_struct_name = construct_program_name(&simf_content.contract_name);
        Ok(SimfContractMeta {
            contract_source_const_name,
            program_struct_name,
            args_struct,
            witness_struct,
            simf_content,
            abi_meta,
        })
    }

    /// Generates code necessary for creating mutant testing using simplex.
    pub fn generate_mutantesting_impl(&self) -> syn::Result<GeneratedMutanTestingTokens> {
        let args_struct_name = &self.args_struct.struct_name;
        let program_name = &self.program_struct_name;

        let fuzzable_program_impl = quote! {
            impl FuzzableProgram<#program_name> for #program_name {
                fn build_program(
                    args: impl Into<simplex::simplicityhl::Arguments>,
                    network: &SimplicityNetwork,
                ) -> (Box<#program_name>, Script) {
                    let prog = #program_name::new(args);
                    let script = prog.get_script_pubkey(network);
                    (Box::new(prog), script)
                }
            }

            impl SimplexProgram for #program_name {
                fn get_program(&self) -> &Program {
                    &self.program
                }

                fn get_compiled_program(args: Arguments) -> CompiledProgram {
                    #program_name::new(args).program.load().unwrap()
                }

                fn get_mut_program(&mut self) -> &mut Program {
                    &mut self.program
                }
            }
        };

        Ok(GeneratedMutanTestingTokens {
            imports: quote! {
                use super::{super::#program_name, #args_struct_name};
                use simplex::mutantesting::FuzzableProgram;
                use simplex::provider::SimplicityNetwork;
                use simplex::program::{Program, SimplexProgram};
                use simplex::simplicityhl::{Arguments, CompiledProgram};
                use simplex::simplicityhl::elements::Script;
            },
            helper_impls: quote! {
                #fuzzable_program_impl
            },
        })
    }
}

impl WitnessField {
    fn new(witness_name: &WitnessName, resolved_type: &ResolvedType) -> syn::Result<Self> {
        let (witness_simf_name, struct_rust_field) = {
            let w_name = witness_name.to_string();
            let r_name = format_ident!("{}", w_name.to_lowercase());
            (w_name, r_name)
        };

        let rust_type = RustType::from_resolved_type(resolved_type)?;

        Ok(Self {
            witness_simf_name,
            struct_rust_field,
            rust_type,
        })
    }

    /// Generate the conversion code from Rust value to Simplicity Value
    fn to_token_stream(&self) -> proc_macro2::TokenStream {
        let witness_name = &self.witness_simf_name;
        let field_name = &self.struct_rust_field;
        let conversion = self
            .rust_type
            .generate_to_simplicity_conversion(&quote! { self.#field_name });

        quote! {
            (
                simplex::simplicityhl::str::WitnessName::from_str_unchecked(#witness_name),
                #conversion
            )
        }
    }
}

impl WitnessStruct {
    /// Generate the implementation for the arguments struct.
    ///
    /// # Errors
    /// Returns a `syn::Result` with an error if the conversion from arguments map fails.
    pub fn generate_arguments_impl(&self) -> syn::Result<GeneratedArgumentTokens> {
        let generated_struct = self.generate_struct_token_stream();
        let struct_name = &self.struct_name;
        let tuples: Vec<proc_macro2::TokenStream> = self.construct_witness_tuples();
        let (arguments_conversion_from_args_map, struct_to_return): (
            proc_macro2::TokenStream,
            proc_macro2::TokenStream,
        ) = self.generate_from_args_conversion_with_param_name("args");
        let rand_mapping: proc_macro2::TokenStream = self.generate_rand_mapping();
        let default_mapping: proc_macro2::TokenStream = self.generate_default_mapping();

        Ok(GeneratedArgumentTokens {
            imports: quote! {
                    use std::collections::HashMap;
                    use simplex::simplicityhl::{Arguments, Value, ResolvedType};
                    use simplex::simplicityhl::value::{UIntValue, ValueInner};
                    use simplex::simplicityhl::num::{NonZeroPow2Usize, U256};
                    use simplex::simplicityhl::str::WitnessName;
                    use simplex::simplicityhl::types::TypeConstructible;
                    use simplex::simplicityhl::value::ValueConstructible;
                    use simplex::program::ArgumentsTrait;
                    use simplex::rand_core::{RngCore};
                    use simplex::rand::Rng;
            },
            struct_token_stream: quote! {
                #generated_struct
            },
            struct_impl: quote! {
                impl #struct_name {
                    /// Build struct from Simplicity Arguments.
                    ///
                    /// # Errors
                    ///
                    /// Returns error if any required witness is missing, has wrong type, or has invalid value.
                    pub fn from_arguments(args: &Arguments) -> Result<Self, String> {
                        #arguments_conversion_from_args_map

                        Ok(#struct_to_return)
                    }

                    /// Generate a random Arguments struct instance using the provided RNG.
                    pub fn generate_arguments_raw<R: RngCore + ?Sized>(rng: &mut R) -> Self
                    {
                        #rand_mapping
                    }
                }

                impl simplex::program::ArgumentsTrait for #struct_name {
                    /// Build Simplicity arguments for contract instantiation.
                    #[must_use]
                    fn build_arguments(&self) -> simplex::simplicityhl::Arguments {
                        simplex::simplicityhl::Arguments::from(HashMap::from([
                            #(#tuples),*
                        ]))
                    }
                }

                impl simplex::serde::Serialize for #struct_name {
                    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                    where
                    S: simplex::serde::Serializer,
                    {
                        self.build_arguments().serialize(serializer)
                    }
                }

                impl<'de> simplex::serde::Deserialize<'de> for #struct_name {
                    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                    where
                    D: simplex::serde::Deserializer<'de>,
                    {
                        let x = simplex::simplicityhl::Arguments::deserialize(deserializer)?;
                        Self::from_arguments(&x).map_err(simplex::serde::de::Error::custom)
                    }
                }

                impl simplex::program::RandomArguments for #struct_name {
                    fn generate_arguments(rng: &mut dyn RngCore) -> simplex::simplicityhl::Arguments
                    {
                        Self::generate_arguments_raw(rng).build_arguments()
                    }
                }

                impl core::default::Default for #struct_name {
                    fn default() -> Self {
                        #default_mapping
                    }
                }

                impl From<#struct_name> for simplex::simplicityhl::Arguments {
                    fn from(val: #struct_name) -> simplex::simplicityhl::Arguments {
                        val.build_arguments()
                    }
                }
            },
        })
    }

    /// Generate the implementation for the witness struct.
    ///
    /// # Errors
    /// Returns a `syn::Result` with an error if the conversion from witness values fails.
    pub fn generate_witness_impl(&self) -> syn::Result<GeneratedWitnessTokens> {
        let generated_struct = self.generate_struct_token_stream();
        let struct_name = &self.struct_name;
        let tuples: Vec<proc_macro2::TokenStream> = self.construct_witness_tuples();
        let (arguments_conversion_from_args_map, struct_to_return): (
            proc_macro2::TokenStream,
            proc_macro2::TokenStream,
        ) = self.generate_from_args_conversion_with_param_name("witness");
        let default_mapping: proc_macro2::TokenStream = self.generate_default_mapping();
        let rand_mapping: proc_macro2::TokenStream = self.generate_rand_mapping();

        Ok(GeneratedWitnessTokens {
            imports: quote! {
                    use std::collections::HashMap;
                    use simplex::simplicityhl::{WitnessValues, Value, ResolvedType};
                    use simplex::simplicityhl::value::{UIntValue, ValueInner};
                    use simplex::simplicityhl::num::{NonZeroPow2Usize, U256};
                    use simplex::simplicityhl::str::WitnessName;
                    use simplex::simplicityhl::types::TypeConstructible;
                    use simplex::simplicityhl::value::ValueConstructible;
                    use simplex::program::WitnessTrait;
                    use simplex::rand_core::{RngCore};
                    use simplex::rand::Rng;
            },
            struct_token_stream: quote! {
                #generated_struct
            },
            struct_impl: quote! {
                impl #struct_name {
                    /// Build struct from Simplicity WitnessValues.
                    ///
                    /// # Errors
                    ///
                    /// Returns error if any required witness is missing, has the wrong type, or has an invalid value.
                    pub fn from_witness(witness: &WitnessValues) -> Result<Self, String> {
                        #arguments_conversion_from_args_map

                        Ok(#struct_to_return)
                    }

                    /// Generate a random Witness struct instance using the provided RNG.
                    pub fn generate_witness_raw<R: RngCore + ?Sized>(rng: &mut R) -> Self
                    {
                        #rand_mapping
                    }
                }

                impl simplex::program::WitnessTrait for #struct_name {
                     /// Build Simplicity witness values for contract execution.
                    #[must_use]
                    fn build_witness(&self) -> simplex::simplicityhl::WitnessValues {
                        simplex::simplicityhl::WitnessValues::from(HashMap::from([
                            #(#tuples),*
                        ]))
                    }
                }

                impl simplex::serde::Serialize for #struct_name {
                    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                    where
                        S: simplex::serde::Serializer,
                    {
                        self.build_witness().serialize(serializer)
                    }
                }

                impl<'de> simplex::serde::Deserialize<'de> for #struct_name {
                    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                    where
                        D: simplex::serde::Deserializer<'de>,
                    {
                        let x = simplex::simplicityhl::WitnessValues::deserialize(deserializer)?;
                        Self::from_witness(&x).map_err(simplex::serde::de::Error::custom)
                    }
                }

                impl simplex::program::RandomWitness for #struct_name {
                    fn generate_witness(rng: &mut dyn RngCore) -> simplex::simplicityhl::WitnessValues
                    {
                        Self::generate_witness_raw(rng).build_witness()
                    }
                }

                impl core::default::Default for #struct_name {
                    fn default() -> Self {
                        #default_mapping
                    }
                }

                impl From<#struct_name> for simplex::simplicityhl::WitnessValues {
                    fn from(val: #struct_name) -> simplex::simplicityhl::WitnessValues {
                        val.build_witness()
                    }
                }
            },
        })
    }

    fn generate_args_struct(contract_name: &str, meta: &Parameters) -> syn::Result<WitnessStruct> {
        let base_name = convert_contract_name_to_struct_name(contract_name);

        Ok(WitnessStruct {
            struct_name: format_ident!("{}Arguments", base_name),
            witness_values: WitnessStruct::generate_witness_fields(meta.iter())?,
        })
    }

    fn generate_witness_struct(contract_name: &str, meta: &WitnessTypes) -> syn::Result<WitnessStruct> {
        let base_name = convert_contract_name_to_struct_name(contract_name);

        Ok(WitnessStruct {
            struct_name: format_ident!("{}Witness", base_name),
            witness_values: WitnessStruct::generate_witness_fields(meta.iter())?,
        })
    }

    fn generate_witness_fields<'a>(
        iter: impl Iterator<Item = (&'a WitnessName, &'a ResolvedType)>,
    ) -> syn::Result<Vec<WitnessField>> {
        iter.map(|(name, resolved_type)| WitnessField::new(name, resolved_type))
            .collect()
    }

    fn generate_struct_token_stream(&self) -> proc_macro2::TokenStream {
        let name = format_ident!("{}", self.struct_name);
        let fields: Vec<proc_macro2::TokenStream> = self
            .witness_values
            .iter()
            .map(|field| {
                let field_name = format_ident!("{}", field.struct_rust_field);
                let field_type = field.rust_type.to_type_token_stream();

                quote! { pub #field_name: #field_type }
            })
            .collect();

        quote! {
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct #name {
                #(#fields),*
            }
        }
    }

    fn generate_rand_mapping(&self) -> proc_macro2::TokenStream {
        let name = format_ident!("{}", self.struct_name);
        let fields: Vec<proc_macro2::TokenStream> = self
            .witness_values
            .iter()
            .map(|field| {
                let field_name = format_ident!("{}", field.struct_rust_field);
                let field_default_value = field.rust_type.get_random_value();
                quote! { #field_name: #field_default_value }
            })
            .collect();
        quote! {
            #name {
                #(#fields),*
            }
        }
    }

    fn generate_default_mapping(&self) -> proc_macro2::TokenStream {
        let name = format_ident!("{}", self.struct_name);
        let fields: Vec<proc_macro2::TokenStream> = self
            .witness_values
            .iter()
            .map(|field| {
                let field_name = format_ident!("{}", field.struct_rust_field);
                let field_default_value = field.rust_type.get_default_value();
                quote! { #field_name: #field_default_value }
            })
            .collect();

        quote! {
            #name {
                #(#fields),*
            }
        }
    }

    #[inline]
    fn construct_witness_tuples(&self) -> Vec<proc_macro2::TokenStream> {
        self.witness_values.iter().map(WitnessField::to_token_stream).collect()
    }

    /// Generate conversion code from Arguments/WitnessValues back to struct fields.
    /// Returns a tuple of (`extraction_code`, `struct_initialization_code`).
    fn generate_from_args_conversion_with_param_name(
        &self,
        param_name: &str,
    ) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
        let param_ident = format_ident!("{}", param_name);
        let field_extractions: Vec<proc_macro2::TokenStream> = self
            .witness_values
            .iter()
            .map(|field| {
                let field_name = &field.struct_rust_field;
                let witness_name = &field.witness_simf_name;
                let extraction = field
                    .rust_type
                    .generate_from_value_extraction(&param_ident, witness_name);

                quote! {
                    let #field_name = #extraction;
                }
            })
            .collect();

        let field_names: Vec<proc_macro2::Ident> = self
            .witness_values
            .iter()
            .map(|field| format_ident!("{}", field.struct_rust_field))
            .collect();

        let extractions = quote! {
            #(#field_extractions)*
        };

        let struct_init = quote! {
            Self {
                #(#field_names),*
            }
        };

        (extractions, struct_init)
    }
}

pub fn construct_program_name(contract_name: &str) -> proc_macro2::Ident {
    let base_name = convert_contract_name_to_struct_name(contract_name);
    format_ident!("{base_name}Program")
}

pub fn convert_contract_name_to_struct_name(contract_name: &str) -> String {
    let words: Vec<String> = contract_name
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|word| {
            let mut chars = word.chars();

            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect();

    words.join("")
}

pub fn convert_contract_name_to_contract_source_const(contract_name: &str) -> proc_macro2::Ident {
    format_ident!("{}_CONTRACT_SOURCE", contract_name.to_uppercase())
}

pub fn convert_contract_name_to_contract_module(contract_name: &str) -> proc_macro2::Ident {
    format_ident!("derived_{}", contract_name)
}
