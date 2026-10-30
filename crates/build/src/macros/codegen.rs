use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use simplicityhl::{AbiMeta, Parameters, ResolvedType, TemplateProgramWitness, WitnessTypes};

use crate::macros::literals::LiteralExtractor;
use crate::macros::parse::SimfContent;
use crate::macros::safe_types::SafeType;
use crate::macros::types::{AllocationType, RustType};

pub struct SimfContractMeta {
    pub contract_source_const_name: Ident,
    pub program_struct_name: Ident,
    pub args_struct: WitnessStruct,
    pub witness_struct: WitnessStruct,
    pub simf_content: SimfContent,
    pub abi_meta: AbiMeta,
}

pub struct GeneratedArgumentTokens {
    pub imports: TokenStream,
    pub struct_token_stream: TokenStream,
    pub struct_impl: TokenStream,
}

pub struct GeneratedWitnessTokens {
    pub imports: TokenStream,
    pub struct_token_stream: TokenStream,
    pub struct_impl: TokenStream,
}

pub struct GeneratedProgramTraitHelperTokens {
    pub imports: TokenStream,
    pub helper_impls: TokenStream,
}

pub struct WitnessField {
    witness_simf_name: String,
    struct_rust_field: Ident,
    rust_type: RustType,
    safe_type: SafeType,
    key_constructor: Ident,
}

pub struct WitnessStruct {
    pub struct_name: Ident,
    witness_values: Vec<WitnessField>,
    has_aliases: bool,
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

    /// Generates code necessary for fuzz testing.
    pub fn generate_program_trait_helpers_impl(&self) -> syn::Result<GeneratedProgramTraitHelperTokens> {
        let args_struct_name = &self.args_struct.struct_name;
        let witness_struct_name = &self.witness_struct.struct_name;
        let program_name = &self.program_struct_name;
        let argument_types = self.args_struct.generate_schema();
        let witness_types = self.witness_struct.generate_schema();
        let literals = LiteralExtractor::extract(&self.simf_content.content)
            .into_iter()
            .map(|literal| {
                let bytes = literal.to_byte_array();
                quote! { U256::from_byte_array([#(#bytes),*]) }
            });

        let program_helpers_impl = quote! {
            impl ProgramFactory<#program_name> for #program_name {
                fn instantiate_program(args: impl Into<Arguments>) -> Box<#program_name> {
                    Box::new(#program_name::new(args))
                }
            }

            impl ProgramSchema for #program_name {
                type Arguments = #args_struct_name;
                type Witness = #witness_struct_name;

                fn argument_types() -> Vec<(TemplateProgramWitness, ResolvedType)> {
                    vec![#(#argument_types),*]
                }

                fn witness_types() -> Vec<(TemplateProgramWitness, ResolvedType)> {
                    vec![#(#witness_types),*]
                }

                fn literals() -> Vec<U256> {
                    vec![#(#literals),*]
                }
            }
        };

        Ok(GeneratedProgramTraitHelperTokens {
            imports: quote! {
                use super::{super::#program_name, #args_struct_name, #witness_struct_name};
                use simplex::program::{Program, ProgramFactory, ProgramSchema};
                use simplex::simplicityhl::{Arguments, ResolvedType, TemplateProgramWitness};
                use simplex::simplicityhl::num::{NonZeroPow2Usize, U256};
                use simplex::simplicityhl::types::TypeConstructible;
            },
            helper_impls: quote! {
                #program_helpers_impl
            },
        })
    }
}

impl WitnessField {
    fn new(
        witness_name: &TemplateProgramWitness,
        resolved_type: &ResolvedType,
        key_constructor: &Ident,
    ) -> syn::Result<Self> {
        let (witness_simf_name, struct_rust_field) = {
            let w_name = witness_name.to_string();
            let r_name = format_ident!("{}", w_name.to_lowercase());
            (w_name, r_name)
        };

        let rust_type = RustType::from_resolved_type(resolved_type)?;
        let safe_type = SafeType::from_resolved_type(resolved_type)?;

        Ok(Self {
            witness_simf_name,
            struct_rust_field,
            rust_type,
            safe_type,
            key_constructor: key_constructor.clone(),
        })
    }

    /// Generate the conversion code from Rust value to Simplicity Value
    fn to_token_stream(&self, struct_name: &Ident, alloc_type: AllocationType) -> TokenStream {
        let witness_name = &self.witness_simf_name;
        let field_name = &self.struct_rust_field;
        let key_constructor = &self.key_constructor;
        let field_access = quote! { #struct_name.#field_name };
        let conversion = self
            .rust_type
            .generate_to_simplicity_conversion(&field_access, alloc_type);

        quote! {
            (
                simplex::simplicityhl::TemplateProgramWitness::#key_constructor(#witness_name),
                #conversion
            )
        }
    }
}

impl WitnessStruct {
    fn new(struct_name: Ident, witness_values: Vec<WitnessField>) -> Self {
        let has_aliases = witness_values.iter().any(|field| field.safe_type.contains_alias());

        Self {
            struct_name,
            witness_values,
            has_aliases,
        }
    }

    /// Generate the implementation for the arguments struct.
    ///
    /// # Errors
    /// Returns a `syn::Result` with an error if the conversion from arguments map fails.
    pub fn generate_arguments_impl(&self) -> syn::Result<GeneratedArgumentTokens> {
        let generated_struct = self.generate_struct_token_stream();
        let (safe_struct, safe_helpers) = self
            .generate_safe_struct(quote! { simplex::simplicityhl::Arguments })
            .unzip();

        let struct_name = &self.struct_name;
        let struct_param = format_ident!("val");
        let copied_tuples: Vec<TokenStream> = self.construct_witness_tuples(&struct_param, AllocationType::Copy);
        let moved_tuples: Vec<TokenStream> = self.construct_witness_tuples(&struct_param, AllocationType::Move);

        let (arguments_conversion_from_args_map, struct_to_return): (TokenStream, TokenStream) =
            self.generate_from_args_conversion_with_param_name("args");
        let default_mapping: TokenStream = self.generate_default_mapping();

        Ok(GeneratedArgumentTokens {
            imports: quote! {
                    use std::collections::HashMap;
                    use simplex::simplicityhl::{Arguments, Value, ResolvedType};
                    use simplex::simplicityhl::value::{UIntValue, ValueInner};
                    use simplex::simplicityhl::num::{NonZeroPow2Usize, U256};
                    use simplex::simplicityhl::{TemplateProgramWitness, WitnessNameToValueMap};
                    use simplex::simplicityhl::types::TypeConstructible;
                    use simplex::simplicityhl::value::ValueConstructible;
            },
            struct_token_stream: quote! {
                #generated_struct
                #safe_struct
            },
            struct_impl: quote! {
                impl #struct_name {
                    /// Build struct from Simplicity `Arguments`.
                    ///
                    /// # Errors
                    ///
                    /// Returns error if any required witness is missing, has the wrong type, or has an invalid value.
                    pub fn from_arguments(args: &Arguments) -> Result<Self, String> {
                        #arguments_conversion_from_args_map

                        Ok(#struct_to_return)
                    }
                }

                impl simplex::serde::Serialize for #struct_name {
                    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                    where
                    S: simplex::serde::Serializer,
                    {
                        let args: Arguments = self.into();
                        args.serialize(serializer)
                    }
                }

                impl<'de> simplex::serde::Deserialize<'de> for #struct_name {
                    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                    where
                    D: simplex::serde::Deserializer<'de>,
                    {
                        let x = Arguments::deserialize(deserializer)?;
                        Self::from_arguments(&x).map_err(simplex::serde::de::Error::custom)
                    }
                }

                impl core::default::Default for #struct_name {
                    fn default() -> Self {
                        #default_mapping
                    }
                }

                impl TryFrom<&Arguments> for #struct_name {
                    type Error = String;

                    fn try_from(args: &Arguments) -> Result<Self, String> {
                        Self::from_arguments(args)
                    }
                }

                impl From<#struct_name> for Arguments {
                    fn from(#struct_param: #struct_name) -> Arguments {
                        Arguments::from_map(HashMap::from([
                            #(#moved_tuples),*
                        ]))
                    }
                }

                impl From<&#struct_name> for Arguments {
                    fn from(#struct_param: &#struct_name) -> Arguments {
                        Arguments::from_map(HashMap::from([
                            #(#copied_tuples),*
                        ]))
                    }
                }

                #safe_helpers
            },
        })
    }

    /// Generate the implementation for the witness struct.
    ///
    /// # Errors
    /// Returns a `syn::Result` with an error if the conversion from witness values fails.
    pub fn generate_witness_impl(&self) -> syn::Result<GeneratedWitnessTokens> {
        let generated_struct = self.generate_struct_token_stream();
        let (safe_struct, safe_helpers) = self
            .generate_safe_struct(quote! { simplex::simplicityhl::WitnessValues })
            .unzip();

        let struct_name = &self.struct_name;
        let struct_param = format_ident!("val");
        let copied_tuples: Vec<TokenStream> = self.construct_witness_tuples(&struct_param, AllocationType::Copy);
        let moved_tuples: Vec<TokenStream> = self.construct_witness_tuples(&struct_param, AllocationType::Move);

        let (arguments_conversion_from_args_map, struct_to_return): (TokenStream, TokenStream) =
            self.generate_from_args_conversion_with_param_name("witness");
        let default_mapping: TokenStream = self.generate_default_mapping();

        Ok(GeneratedWitnessTokens {
            imports: quote! {
                    use std::collections::HashMap;
                    use simplex::simplicityhl::{WitnessValues, Value, ResolvedType};
                    use simplex::simplicityhl::value::{UIntValue, ValueInner};
                    use simplex::simplicityhl::num::{NonZeroPow2Usize, U256};
                    use simplex::simplicityhl::{TemplateProgramWitness, WitnessNameToValueMap};
                    use simplex::simplicityhl::types::TypeConstructible;
                    use simplex::simplicityhl::value::ValueConstructible;
            },
            struct_token_stream: quote! {
                #generated_struct
                #safe_struct
            },
            struct_impl: quote! {
                impl #struct_name {
                    /// Build struct from Simplicity `WitnessValues`.
                    ///
                    /// # Errors
                    ///
                    /// Returns error if any required witness is missing, has the wrong type, or has an invalid value.
                    pub fn from_witness(witness: &WitnessValues) -> Result<Self, String> {
                        #arguments_conversion_from_args_map

                        Ok(#struct_to_return)
                    }
                }

                impl simplex::serde::Serialize for #struct_name {
                    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                    where
                        S: simplex::serde::Serializer,
                    {
                        let wit: WitnessValues = self.into();
                        wit.serialize(serializer)
                    }
                }

                impl<'de> simplex::serde::Deserialize<'de> for #struct_name {
                    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                    where
                        D: simplex::serde::Deserializer<'de>,
                    {
                        let x = WitnessValues::deserialize(deserializer)?;
                        Self::from_witness(&x).map_err(simplex::serde::de::Error::custom)
                    }
                }

                impl core::default::Default for #struct_name {
                    fn default() -> Self {
                        #default_mapping
                    }
                }

                impl TryFrom<&WitnessValues> for #struct_name {
                    type Error = String;

                    fn try_from(witness: &WitnessValues) -> Result<Self, String> {
                        Self::from_witness(witness)
                    }
                }

                impl From<#struct_name> for WitnessValues {
                    fn from(#struct_param: #struct_name) -> WitnessValues {
                        WitnessValues::from_map(HashMap::from([
                            #(#moved_tuples),*
                        ]))
                    }
                }

                impl From<&#struct_name> for WitnessValues {
                    fn from(#struct_param: &#struct_name) -> WitnessValues {
                        WitnessValues::from_map(HashMap::from([
                            #(#copied_tuples),*
                        ]))
                    }
                }

                #safe_helpers
            },
        })
    }

    fn generate_args_struct(contract_name: &str, meta: &Parameters) -> syn::Result<WitnessStruct> {
        let base_name = convert_contract_name_to_struct_name(contract_name);

        Ok(Self::new(
            format_ident!("{}Arguments", base_name),
            Self::generate_witness_fields(meta.iter(), &format_ident!("parameter_from_str"))?,
        ))
    }

    fn generate_witness_struct(contract_name: &str, meta: &WitnessTypes) -> syn::Result<WitnessStruct> {
        let base_name = convert_contract_name_to_struct_name(contract_name);
        Ok(Self::new(
            format_ident!("{}Witness", base_name),
            Self::generate_witness_fields(meta.iter(), &format_ident!("witness_from_str"))?,
        ))
    }

    fn generate_witness_fields<'a>(
        iter: impl Iterator<Item = (&'a TemplateProgramWitness, &'a ResolvedType)>,
        key_constructor: &Ident,
    ) -> syn::Result<Vec<WitnessField>> {
        iter.map(|(name, resolved_type)| WitnessField::new(name, resolved_type, key_constructor))
            .collect()
    }

    fn generate_struct_token_stream(&self) -> TokenStream {
        let name = format_ident!("{}", self.struct_name);
        let fields: Vec<TokenStream> = self
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

    fn generate_safe_struct(&self, convert_to_type: TokenStream) -> Option<(TokenStream, TokenStream)> {
        if !self.has_aliases {
            return None;
        }

        let name = &self.struct_name;
        let safe_name = format_ident!("{name}Safe");

        let fields = self.witness_values.iter().map(|field| {
            let field_name = &field.struct_rust_field;
            let field_type = field.safe_type.to_type_token_stream();
            quote! { pub #field_name: #field_type }
        });

        let conversions = self.witness_values.iter().map(|field| {
            let field_name = &field.struct_rust_field;
            let conversion = field.safe_type.to_concrete_value(&quote! { &value.#field_name });
            quote! { #field_name: #conversion }
        });

        let defaults = self.witness_values.iter().map(|field| {
            let field_name = &field.struct_rust_field;
            let default = field.safe_type.get_default_value();
            quote! { #field_name: #default }
        });

        let (struct_tokens, helper_tokens) = (
            quote! {
                #[derive(Debug, Clone, PartialEq, Eq)]
                pub struct #safe_name {
                    #(#fields),*
                }
            },
            quote! {
                impl core::default::Default for #safe_name {
                    fn default() -> Self {
                        Self { #(#defaults),* }
                    }
                }

                impl From<&#safe_name> for #name {
                    fn from(value: &#safe_name) -> Self {
                        Self { #(#conversions),* }
                    }
                }

                impl From<#safe_name> for #name {
                    fn from(value: #safe_name) -> Self {
                        Self::from(&value)
                    }
                }

                impl From<&#safe_name> for #convert_to_type {
                    fn from(value: &#safe_name) -> Self {
                        Self::from(#name::from(value))
                    }
                }

                impl From<#safe_name> for #convert_to_type {
                    fn from(value: #safe_name) -> Self {
                        Self::from(#name::from(value))
                    }
                }
            },
        );

        Some((struct_tokens, helper_tokens))
    }

    fn generate_schema(&self) -> Vec<TokenStream> {
        let mut fields: Vec<_> = self.witness_values.iter().collect();
        fields.sort_unstable_by(|a, b| a.witness_simf_name.cmp(&b.witness_simf_name));

        fields
            .into_iter()
            .map(|field| {
                let name = &field.witness_simf_name;
                let key_constructor = &field.key_constructor;
                let ty = field.rust_type.generate_simplicity_type_construction();

                quote! { (TemplateProgramWitness::#key_constructor(#name), #ty) }
            })
            .collect()
    }

    fn generate_default_mapping(&self) -> TokenStream {
        let name = format_ident!("{}", self.struct_name);
        let fields: Vec<TokenStream> = self
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
    fn construct_witness_tuples(&self, struct_name: &Ident, alloc_type: AllocationType) -> Vec<TokenStream> {
        self.witness_values
            .iter()
            .map(|wit_field| wit_field.to_token_stream(struct_name, alloc_type))
            .collect()
    }

    /// Generate conversion code from Arguments/WitnessValues back to struct fields.
    /// Returns a tuple of (`extraction_code`, `struct_initialization_code`).
    fn generate_from_args_conversion_with_param_name(&self, param_name: &str) -> (TokenStream, TokenStream) {
        let param_ident = format_ident!("{}", param_name);
        let field_extractions: Vec<TokenStream> = self
            .witness_values
            .iter()
            .map(|field| {
                let field_name = &field.struct_rust_field;
                let witness_name = &field.witness_simf_name;
                let extraction =
                    field
                        .rust_type
                        .generate_from_value_extraction(&param_ident, witness_name, &field.key_constructor);

                quote! {
                    let #field_name = #extraction;
                }
            })
            .collect();

        let field_names: Vec<Ident> = self
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

pub fn construct_program_name(contract_name: &str) -> Ident {
    let base_name = convert_contract_name_to_struct_name(contract_name);
    format_ident!("{base_name}Program")
}

pub fn convert_contract_name_to_struct_name(contract_name: &str) -> String {
    let starts_with_underscore = contract_name.starts_with('_');
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

    let joined = words.join("");

    if starts_with_underscore {
        format!("_{joined}")
    } else {
        joined
    }
}

pub fn convert_contract_name_to_contract_source_const(contract_name: &str) -> Ident {
    format_ident!("{}_CONTRACT_SOURCE", contract_name.to_uppercase())
}

pub fn convert_contract_name_to_contract_module(contract_name: &str) -> Ident {
    format_ident!("derived_{}", contract_name)
}

#[cfg(test)]
mod tests {
    use simplicityhl::types::TypeConstructible;
    use simplicityhl::{ResolvedType, TemplateProgramWitness};

    use super::{WitnessStruct, convert_contract_name_to_struct_name};

    #[test]
    fn struct_names_preserve_a_leading_identifier_underscore() {
        assert_eq!(convert_contract_name_to_struct_name("_9_lives"), "_9Lives");
    }

    #[test]
    fn schema_is_independent_of_metadata_order() {
        let metadata = [
            (TemplateProgramWitness::witness_from_str("Z"), ResolvedType::u16()),
            (TemplateProgramWitness::witness_from_str("A"), ResolvedType::boolean()),
            (TemplateProgramWitness::witness_from_str("M"), ResolvedType::u32()),
        ];

        let key_constructor = quote::format_ident!("witness_from_str");
        let iter = metadata.iter().map(|(name, ty)| (name, ty));

        let mut fields = WitnessStruct::new(
            quote::format_ident!("SeededFields"),
            WitnessStruct::generate_witness_fields(iter, &key_constructor).unwrap(),
        );

        let schema = |fields: &WitnessStruct| -> String {
            fields
                .generate_schema()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        };
        let ideal_schema = schema(&fields);

        fields.witness_values.reverse();
        assert_eq!(ideal_schema, schema(&fields));

        let (idx_a, idx_m, idx_z) = (
            ideal_schema.find("\"A\"").unwrap(),
            ideal_schema.find("\"M\"").unwrap(),
            ideal_schema.find("\"Z\"").unwrap(),
        );
        assert!(idx_a < idx_m && idx_m < idx_z);
    }
}
