use std::fmt::Display;

use quote::quote;

use simplicityhl::ResolvedType;

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RustType {
    Bool,
    U8,
    U16,
    U32,
    U64,
    U128,
    U256Array,
    Array(Box<RustType>, usize),
    Tuple(Vec<RustType>),
    Either(Box<RustType>, Box<RustType>),
    Option(Box<RustType>),
}

#[derive(Debug, Clone, Copy)]
enum RustTypeContext {
    Array,
    Tuple,
    Either,
    EitherLeft,
    EitherRight,
    Option,
}

impl Display for RustTypeContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            RustTypeContext::Array => "array element".to_string(),
            RustTypeContext::Tuple => "tuple element".to_string(),
            RustTypeContext::EitherLeft => "left either branch".to_string(),
            RustTypeContext::EitherRight => "right either branch".to_string(),
            RustTypeContext::Option => "option element".to_string(),
            RustTypeContext::Either => "either element".to_string(),
        };
        write!(f, "{str}")
    }
}

impl RustTypeContext {
    fn is_deref_needed(&self) -> bool {
        match self {
            RustTypeContext::Array => false,
            RustTypeContext::Tuple => false,
            RustTypeContext::Either => true,
            RustTypeContext::EitherLeft => true,
            RustTypeContext::EitherRight => true,
            RustTypeContext::Option => true,
        }
    }
}

impl RustType {
    pub fn from_resolved_type(ty: &ResolvedType) -> syn::Result<Self> {
        use simplicityhl::types::{TypeInner, UIntType};

        match ty.as_inner() {
            TypeInner::Boolean => Ok(RustType::Bool),
            TypeInner::UInt(uint_ty) => match uint_ty {
                UIntType::U1 => Ok(RustType::Bool),
                UIntType::U2 | UIntType::U4 | UIntType::U8 => Ok(RustType::U8),
                UIntType::U16 => Ok(RustType::U16),
                UIntType::U32 => Ok(RustType::U32),
                UIntType::U64 => Ok(RustType::U64),
                UIntType::U128 => Ok(RustType::U128),
                UIntType::U256 => Ok(RustType::U256Array),
            },
            TypeInner::Either(left, right) => {
                let left_ty = Self::from_resolved_type(left)?;
                let right_ty = Self::from_resolved_type(right)?;
                Ok(RustType::Either(Box::new(left_ty), Box::new(right_ty)))
            }
            TypeInner::Option(inner) => {
                let inner_ty = Self::from_resolved_type(inner)?;
                Ok(RustType::Option(Box::new(inner_ty)))
            }
            TypeInner::Tuple(elements) => {
                let element_types: syn::Result<Vec<_>> = elements.iter().map(|e| Self::from_resolved_type(e)).collect();
                Ok(RustType::Tuple(element_types?))
            }
            TypeInner::Array(element, size) => {
                let element_ty = Self::from_resolved_type(element)?;
                Ok(RustType::Array(Box::new(element_ty), *size))
            }
            TypeInner::List(_, _) => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "List types are not yet supported in macro conversions",
            )),
            _ => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "Unsupported type in macro conversions",
            )),
        }
    }

    /// Generate the Rust type as a `TokenStream` for struct field declarations
    pub fn to_type_token_stream(&self) -> proc_macro2::TokenStream {
        match self {
            RustType::Bool => quote! { bool },
            RustType::U8 => quote! { u8 },
            RustType::U16 => quote! { u16 },
            RustType::U32 => quote! { u32 },
            RustType::U64 => quote! { u64 },
            RustType::U128 => quote! { u128 },
            RustType::U256Array => quote! { [u8; 32] },
            RustType::Array(element, size) => {
                let element_ty = element.to_type_token_stream();
                quote! { [#element_ty; #size] }
            }
            RustType::Tuple(elements) => {
                let element_types: Vec<_> = elements.iter().map(RustType::to_type_token_stream).collect();
                quote! { (#(#element_types),*) }
            }
            RustType::Either(left, right) => {
                let left_ty = left.to_type_token_stream();
                let right_ty = right.to_type_token_stream();
                quote! { simplex::either::Either<#left_ty, #right_ty> }
            }
            RustType::Option(inner) => {
                let inner_ty = inner.to_type_token_stream();
                quote! { Option<#inner_ty> }
            }
        }
    }

    pub fn generate_to_simplicity_conversion(&self, value_expr: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        self.generate_to_simplicity_conversion_inner(value_expr, None)
    }

    fn generate_to_simplicity_conversion_inner(
        &self,
        value_expr: &proc_macro2::TokenStream,
        prev_type: Option<RustTypeContext>,
    ) -> proc_macro2::TokenStream {
        let deref = {
            if let Some(type_context) = prev_type
                && type_context.is_deref_needed()
            {
                quote! { * }
            } else {
                quote! {}
            }
        };
        match self {
            RustType::Bool => {
                quote! { Value::from(#deref #value_expr) }
            }
            RustType::U8 => {
                quote! { Value::from(UIntValue::U8(#deref #value_expr)) }
            }
            RustType::U16 => {
                quote! { Value::from(UIntValue::U16(#deref #value_expr)) }
            }
            RustType::U32 => {
                quote! { Value::from(UIntValue::U32(#deref #value_expr)) }
            }
            RustType::U64 => {
                quote! { Value::from(UIntValue::U64(#deref #value_expr)) }
            }
            RustType::U128 => {
                quote! { Value::from(UIntValue::U128(#deref #value_expr)) }
            }
            RustType::U256Array => {
                quote! { Value::from(UIntValue::U256(U256::from_byte_array(#deref #value_expr))) }
            }
            RustType::Array(element, size) => {
                let indices: Vec<_> = (0..*size).map(syn::Index::from).collect();
                let element_conversions: Vec<_> = indices
                    .iter()
                    .map(|idx| {
                        let elem_expr = quote! { #value_expr[#idx] };
                        element.generate_to_simplicity_conversion_inner(&elem_expr, Some(RustTypeContext::Array))
                    })
                    .collect();

                let elem_ty_generation = element.generate_simplicity_type_construction();

                quote! {
                    {
                        let elements = [#(#element_conversions),*];
                        Value::array(elements, #elem_ty_generation)
                    }
                }
            }
            RustType::Tuple(elements) => {
                if elements.is_empty() {
                    quote! { Value::unit() }
                } else {
                    let tuple_conversions = elements.iter().enumerate().map(|(i, elem_ty)| {
                        let idx = syn::Index::from(i);
                        let elem_expr = quote! { #value_expr.#idx };
                        elem_ty.generate_to_simplicity_conversion_inner(&elem_expr, Some(RustTypeContext::Tuple))
                    });

                    quote! {
                        Value::tuple([#(#tuple_conversions),*])
                    }
                }
            }
            RustType::Either(left, right) => {
                let left_conv = left
                    .generate_to_simplicity_conversion_inner(&quote! { left_val }, Some(RustTypeContext::EitherLeft));
                let right_conv = right
                    .generate_to_simplicity_conversion_inner(&quote! { right_val }, Some(RustTypeContext::EitherRight));
                let left_ty = left.generate_simplicity_type_construction();
                let right_ty = right.generate_simplicity_type_construction();

                quote! {
                    match &#value_expr {
                        simplex::either::Either::Left(left_val) => {
                            Value::left(
                                #left_conv,
                                #right_ty
                            )
                        }
                        simplex::either::Either::Right(right_val) => {
                            Value::right(
                                #left_ty,
                                #right_conv
                            )
                        }
                    }
                }
            }
            RustType::Option(inner) => {
                let inner_conv =
                    inner.generate_to_simplicity_conversion_inner(&quote! { inner_val }, Some(RustTypeContext::Option));
                let inner_ty = inner.generate_simplicity_type_construction();

                quote! {
                    match &#value_expr {
                        None => {
                            Value::none(#inner_ty)
                        }
                        Some(inner_val) => {
                            Value::some(#inner_conv)
                        }
                    }
                }
            }
        }
    }

    pub fn generate_simplicity_type_construction(&self) -> proc_macro2::TokenStream {
        match self {
            RustType::Bool => {
                quote! { ResolvedType::boolean() }
            }
            RustType::U8 => {
                quote! { ResolvedType::u8() }
            }
            RustType::U16 => {
                quote! { ResolvedType::u16() }
            }
            RustType::U32 => {
                quote! { ResolvedType::u32() }
            }
            RustType::U64 => {
                quote! { ResolvedType::u64() }
            }
            RustType::U128 => {
                quote! { ResolvedType::u128() }
            }
            RustType::U256Array => {
                quote! { ResolvedType::u256() }
            }
            RustType::Array(element, size) => {
                let elem_ty = element.generate_simplicity_type_construction();
                quote! { ResolvedType::array(#elem_ty, #size) }
            }
            RustType::Tuple(elements) => {
                let elem_types: Vec<_> = elements
                    .iter()
                    .map(RustType::generate_simplicity_type_construction)
                    .collect();
                quote! { ResolvedType::tuple([#(#elem_types),*]) }
            }
            RustType::Either(left, right) => {
                let left_ty = left.generate_simplicity_type_construction();
                let right_ty = right.generate_simplicity_type_construction();
                quote! { ResolvedType::either(#left_ty, #right_ty) }
            }
            RustType::Option(inner) => {
                let inner_ty = inner.generate_simplicity_type_construction();
                quote! { ResolvedType::option(#inner_ty) }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn generate_from_value_extraction(
        &self,
        args_expr: &proc_macro2::Ident,
        witness_name: &str,
    ) -> proc_macro2::TokenStream {
        match self {
            RustType::Bool => {
                quote! {
                    {
                        let witness_name = WitnessName::from_str_unchecked(#witness_name);
                        let value = #args_expr
                            .get(&witness_name)
                            .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
                        match value.inner() {
                            simplex::simplicityhl::value::ValueInner::Boolean(b) => *b,
                            _ => return Err(format!("Wrong type for {}: expected bool", #witness_name)),
                        }
                    }
                }
            }
            RustType::U8 => {
                quote! {
                    {
                        let witness_name = WitnessName::from_str_unchecked(#witness_name);
                        let value = #args_expr
                            .get(&witness_name)
                            .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
                        match value.inner() {
                            simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U8(v)) => *v,
                            _ => return Err(format!("Wrong type for {}: expected U8", #witness_name)),
                        }
                    }
                }
            }
            RustType::U16 => {
                quote! {
                    {
                        let witness_name = WitnessName::from_str_unchecked(#witness_name);
                        let value = #args_expr
                            .get(&witness_name)
                            .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
                        match value.inner() {
                            simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U16(v)) => *v,
                            _ => return Err(format!("Wrong type for {}: expected U16", #witness_name)),
                        }
                    }
                }
            }
            RustType::U32 => {
                quote! {
                    {
                        let witness_name = WitnessName::from_str_unchecked(#witness_name);
                        let value = #args_expr
                            .get(&witness_name)
                            .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
                        match value.inner() {
                            simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U32(v)) => *v,
                            _ => return Err(format!("Wrong type for {}: expected U32", #witness_name)),
                        }
                    }
                }
            }
            RustType::U64 => {
                quote! {
                    {
                        let witness_name = WitnessName::from_str_unchecked(#witness_name);
                        let value = #args_expr
                            .get(&witness_name)
                            .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
                        match value.inner() {
                            simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U64(v)) => *v,
                            _ => return Err(format!("Wrong type for {}: expected U64", #witness_name)),
                        }
                    }
                }
            }
            RustType::U128 => {
                quote! {
                    {
                        let witness_name = WitnessName::from_str_unchecked(#witness_name);
                        let value = #args_expr
                            .get(&witness_name)
                            .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
                        match value.inner() {
                            simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U128(v)) => *v,
                            _ => return Err(format!("Wrong type for {}: expected U128", #witness_name)),
                        }
                    }
                }
            }
            RustType::U256Array => {
                quote! {
                    {
                        let witness_name = WitnessName::from_str_unchecked(#witness_name);
                        let value = #args_expr
                            .get(&witness_name)
                            .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
                        match value.inner() {
                            simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U256(u256)) => u256.to_byte_array(),
                            _ => return Err(format!("Wrong type for {}: expected U256", #witness_name)),
                        }
                    }
                }
            }
            RustType::Array(element, size) => {
                let elem_extraction =
                    (0..*size).map(|i| element.generate_inline_array_element_extraction(&quote! { arr_value }, i));

                quote! {
                    {
                        let witness_name = WitnessName::from_str_unchecked(#witness_name);
                        let value = #args_expr
                            .get(&witness_name)
                            .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
                        match value.inner() {
                            simplex::simplicityhl::value::ValueInner::Array(arr_value) => {
                                if arr_value.len() != #size {
                                    return Err(format!("Wrong array length for {}: expected {}, got {}", #witness_name, #size, arr_value.len()));
                                }
                                [#(#elem_extraction),*]
                            }
                            _ => return Err(format!("Wrong type for {}: expected Array", #witness_name)),
                        }
                    }
                }
            }
            RustType::Tuple(elements) => {
                let elem_extractions: Vec<_> = elements
                    .iter()
                    .enumerate()
                    .map(|(i, elem_ty)| elem_ty.generate_inline_tuple_element_extraction(&quote! { tuple_value }, i))
                    .collect();
                let elements_len = elements.len();

                quote! {
                    {
                        let witness_name = WitnessName::from_str_unchecked(#witness_name);
                        let value = #args_expr
                            .get(&witness_name)
                            .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
                        match value.inner() {
                            simplex::simplicityhl::value::ValueInner::Tuple(tuple_value) => {
                                if tuple_value.len() != #elements_len {
                                    return Err(format!("Wrong tuple length for {}", #witness_name));
                                }
                                (#(#elem_extractions),*)
                            }
                            _ => return Err(format!("Wrong type for {}: expected Tuple", #witness_name)),
                        }
                    }
                }
            }
            RustType::Either(left, right) => {
                let left_extraction = left.generate_inline_either_extraction(&quote! { left_val });
                let right_extraction = right.generate_inline_either_extraction(&quote! { right_val });

                quote! {
                    {
                        let witness_name = WitnessName::from_str_unchecked(#witness_name);
                        let value = #args_expr
                            .get(&witness_name)
                            .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
                        match value.inner() {
                            simplex::simplicityhl::value::ValueInner::Either(either_val) => {
                                match either_val {
                                    simplex::either::Either::Left(left_val) => {
                                        simplex::either::Either::Left(#left_extraction)
                                    }
                                    simplex::either::Either::Right(right_val) => {
                                        simplex::either::Either::Right(#right_extraction)
                                    }
                                }
                            }
                            _ => return Err(format!("Wrong type for {}: expected Either", #witness_name)),
                        }
                    }
                }
            }
            RustType::Option(inner) => {
                let inner_extraction = inner.generate_inline_either_extraction(&quote! { some_val });

                quote! {
                    {
                        let witness_name = WitnessName::from_str_unchecked(#witness_name);
                        let value = #args_expr
                            .get(&witness_name)
                            .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
                        match value.inner() {
                            simplex::simplicityhl::value::ValueInner::Option(opt_val) => {
                                match opt_val {
                                    None => None,
                                    Some(some_val) => Some(#inner_extraction),
                                }
                            }
                            _ => return Err(format!("Wrong type for {}: expected Option", #witness_name)),
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn generate_value_extraction_from_expr(
        &self,
        value_expr: &proc_macro2::TokenStream,
        context: RustTypeContext,
    ) -> proc_macro2::TokenStream {
        let context = format!("{context:?}");
        match self {
            RustType::Bool => quote! {
                match #value_expr.inner() {
                    simplex::simplicityhl::value::ValueInner::Boolean(b) => *b,
                    _ => return Err(format!("Wrong type for {}: expected bool", #context)),
                }
            },
            RustType::U8 => quote! {
                match #value_expr.inner() {
                    simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U8(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U8", #context)),
                }
            },
            RustType::U16 => quote! {
                match #value_expr.inner() {
                    simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U16(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U16", #context)),
                }
            },
            RustType::U32 => quote! {
                match #value_expr.inner() {
                    simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U32(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U32", #context)),
                }
            },
            RustType::U64 => quote! {
                match #value_expr.inner() {
                    simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U64(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U64", #context)),
                }
            },
            RustType::U128 => quote! {
                match #value_expr.inner() {
                    simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U128(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U128", #context)),
                }
            },
            RustType::U256Array => quote! {
                match #value_expr.inner() {
                    simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U256(u256)) => u256.to_byte_array(),
                    _ => return Err(format!("Wrong type for {}: expected U256", #context)),
                }
            },
            RustType::Array(element, size) => {
                let elem_extractions: Vec<_> = (0..*size)
                    .map(|i| {
                        element.generate_value_extraction_from_expr(&quote! { arr_val[#i] }, RustTypeContext::Array)
                    })
                    .collect();

                quote! {
                    match #value_expr.inner() {
                        simplex::simplicityhl::value::ValueInner::Array(arr_val) => {
                            if arr_val.len() != #size {
                                return Err(format!("Wrong array length for {}: expected {}, got {}", #context, #size, arr_val.len()));
                            }
                            [#(#elem_extractions),*]
                        }
                        _ => return Err(format!("Wrong type for {}: expected Array", #context)),
                    }
                }
            }
            RustType::Tuple(elements) => {
                let tuple_len = elements.len();
                let elem_extractions: Vec<_> = elements
                    .iter()
                    .enumerate()
                    .map(|(i, elem_ty)| {
                        elem_ty.generate_value_extraction_from_expr(&quote! { tuple_val[#i] }, RustTypeContext::Tuple)
                    })
                    .collect();

                quote! {
                    match #value_expr.inner() {
                        simplex::simplicityhl::value::ValueInner::Tuple(tuple_val) => {
                            if tuple_val.len() != #tuple_len {
                                return Err(format!("Wrong tuple length for {}", #context));
                            }
                            (#(#elem_extractions),*)
                        }
                        _ => return Err(format!("Wrong type for {}: expected Tuple", #context)),
                    }
                }
            }
            RustType::Either(left, right) => {
                let left_extraction =
                    left.generate_value_extraction_from_expr(&quote! { left_val }, RustTypeContext::EitherLeft);
                let right_extraction =
                    right.generate_value_extraction_from_expr(&quote! { right_val }, RustTypeContext::EitherRight);

                quote! {
                    match #value_expr.inner() {
                        simplex::simplicityhl::value::ValueInner::Either(either_val) => {
                            match either_val {
                                simplex::either::Either::Left(left_val) => {
                                    simplex::either::Either::Left(#left_extraction)
                                }
                                simplex::either::Either::Right(right_val) => {
                                    simplex::either::Either::Right(#right_extraction)
                                }
                            }
                        }
                        _ => return Err(format!("Wrong type for {}: expected Either", #context)),
                    }
                }
            }
            RustType::Option(inner) => {
                let inner_extraction =
                    inner.generate_value_extraction_from_expr(&quote! { some_val }, RustTypeContext::Option);

                quote! {
                    match #value_expr.inner() {
                        simplex::simplicityhl::value::ValueInner::Option(opt_val) => {
                            match opt_val {
                                None => None,
                                Some(some_val) => Some(#inner_extraction),
                            }
                        }
                        _ => return Err(format!("Wrong type for {}: expected Option", #context)),
                    }
                }
            }
        }
    }

    fn generate_inline_array_element_extraction(
        &self,
        arr_expr: &proc_macro2::TokenStream,
        index: usize,
    ) -> proc_macro2::TokenStream {
        self.generate_value_extraction_from_expr(&quote! { #arr_expr[#index] }, RustTypeContext::Array)
    }

    fn generate_inline_tuple_element_extraction(
        &self,
        tuple_expr: &proc_macro2::TokenStream,
        index: usize,
    ) -> proc_macro2::TokenStream {
        self.generate_value_extraction_from_expr(&quote! { #tuple_expr[#index] }, RustTypeContext::Tuple)
    }

    fn generate_inline_either_extraction(&self, val_expr: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        let extraction = self.generate_value_extraction_from_expr(val_expr, RustTypeContext::Either);
        quote! {
            #extraction
        }
    }
}
