use std::collections::HashMap;
use std::fmt::Display;

use proc_macro2::TokenStream;
use quote::quote;

use simplicityhl::either::Either;
use simplicityhl::num::U256;
use simplicityhl::types::{TypeInner, UIntType};
use simplicityhl::value::{UIntValue, ValueConstructible, ValueInner};
use simplicityhl::{Arguments, Parameters, ResolvedType, Value};

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RustType {
    Bool,
    U1,
    U2,
    U4,
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
    List(Box<RustType>, usize),
}

#[derive(Debug, Clone, Copy)]
enum RustTypeContext {
    Root,
    Array,
    Tuple,
    EitherLeft,
    EitherRight,
    Option,
    List,
}

impl Display for RustTypeContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            RustTypeContext::Root => "root element".to_string(),
            RustTypeContext::Array => "array element".to_string(),
            RustTypeContext::Tuple => "tuple element".to_string(),
            RustTypeContext::EitherLeft => "left either branch".to_string(),
            RustTypeContext::EitherRight => "right either branch".to_string(),
            RustTypeContext::Option => "option element".to_string(),
            RustTypeContext::List => "list element".to_string(),
        };
        write!(f, "{str}")
    }
}

impl RustTypeContext {
    fn is_deref_needed(&self) -> bool {
        match self {
            RustTypeContext::Array | RustTypeContext::Tuple | RustTypeContext::List | RustTypeContext::Root => false,
            RustTypeContext::EitherLeft | RustTypeContext::EitherRight | RustTypeContext::Option => true,
        }
    }
}

/// The single source of truth for Simplex's default arguments.
///
/// [`default_tokens`] transcribes whatever this returns into the generated `Default` impl,
/// so the values a contract is dry-run with and the values its generated struct defaults to
/// are the same by construction.
///
/// # Errors
/// Returns a `syn::Error` if the type is not supported by the macro conversions.
pub fn default_value(ty: &ResolvedType) -> syn::Result<Value> {
    match ty.as_inner() {
        TypeInner::Boolean => Ok(Value::from(false)),
        TypeInner::UInt(uint_ty) => Ok(Value::from(default_uint(*uint_ty))),
        TypeInner::Option(inner) => Ok(Value::none(inner.as_ref().clone())),
        TypeInner::Either(left, right) => Ok(Value::left(default_value(left)?, right.as_ref().clone())),
        TypeInner::Tuple(elements) => {
            let elements: syn::Result<Vec<_>> = elements.iter().map(|el| default_value(el)).collect();
            Ok(Value::tuple(elements?))
        }
        TypeInner::Array(element, size) => {
            let elements: syn::Result<Vec<_>> = (0..*size).map(|_| default_value(element)).collect();
            Ok(Value::array(elements?, element.as_ref().clone()))
        }
        TypeInner::List(element, bound) => Ok(Value::list(std::iter::empty(), element.as_ref().clone(), *bound)),
        _ => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "Unsupported type in macro conversions",
        )),
    }
}

/// Builds a full set of default arguments for a program's parameters.
///
/// These are the values a contract is dry-run with when no real arguments exist yet.
///
/// # Errors
/// Returns a `syn::Error` if any parameter has a type the macro conversions do not support.
pub fn default_arguments(parameters: &Parameters) -> syn::Result<Arguments> {
    parameters
        .iter()
        .map(|(name, ty)| Ok((name.clone(), default_value(ty)?)))
        .collect::<syn::Result<HashMap<_, _>>>()
        .map(Arguments::from)
}

fn default_uint(ty: UIntType) -> UIntValue {
    match ty {
        UIntType::U1 => UIntValue::U1(0),
        UIntType::U2 => UIntValue::U2(0),
        UIntType::U4 => UIntValue::U4(0),
        UIntType::U8 => UIntValue::U8(0),
        UIntType::U16 => UIntValue::U16(0),
        UIntType::U32 => UIntValue::U32(0),
        UIntType::U64 => UIntValue::U64(0),
        UIntType::U128 => UIntValue::U128(0),
        UIntType::U256 => UIntValue::U256(U256::MIN),
    }
}

/// Transcribes a value into the Rust literal its generated struct field expects.
///
/// Mechanical by design: every default *decision* lives in [`default_value`], so the two
/// cannot drift.
pub fn default_tokens(value: &Value) -> TokenStream {
    match value.inner() {
        ValueInner::Boolean(b) => quote! { #b },
        ValueInner::UInt(uint) => uint_tokens(*uint),
        ValueInner::Option(None) => quote! { None },
        ValueInner::Option(Some(inner)) => {
            let inner = default_tokens(inner);
            quote! { Some(#inner) }
        }
        ValueInner::Either(Either::Left(left)) => {
            let left = default_tokens(left);
            quote! { simplex::either::Either::Left(#left) }
        }
        ValueInner::Either(Either::Right(right)) => {
            let right = default_tokens(right);
            quote! { simplex::either::Either::Right(#right) }
        }
        ValueInner::Tuple(elements) => {
            let elements = elements.iter().map(default_tokens);
            quote! { (#(#elements),*) }
        }
        ValueInner::Array(elements) => array_tokens(&elements.iter().map(default_tokens).collect::<Vec<_>>()),
        // The generated field is a collection; defaults are always the empty list.
        ValueInner::List(..) => quote! { Default::default() },
    }
}

fn uint_tokens(uint: UIntValue) -> TokenStream {
    match uint {
        UIntValue::U1(v) | UIntValue::U2(v) | UIntValue::U4(v) | UIntValue::U8(v) => quote! { #v },
        UIntValue::U16(v) => quote! { #v },
        UIntValue::U32(v) => quote! { #v },
        UIntValue::U64(v) => quote! { #v },
        UIntValue::U128(v) => quote! { #v },
        UIntValue::U256(v) => array_tokens(&v.to_byte_array().iter().map(|b| quote! { #b }).collect::<Vec<_>>()),
    }
}

/// Collapses uniform arrays to `[elem; N]` so a 32-byte default stays readable.
fn array_tokens(elements: &[TokenStream]) -> TokenStream {
    let len = proc_macro2::Literal::usize_unsuffixed(elements.len());
    let uniform = elements.windows(2).all(|w| w[0].to_string() == w[1].to_string());

    match elements.first() {
        Some(first) if uniform => quote! { [#first; #len] },
        _ => quote! { [#(#elements),*] },
    }
}

impl RustType {
    pub fn from_resolved_type(ty: &ResolvedType) -> syn::Result<Self> {
        match ty.as_inner() {
            TypeInner::Boolean => Ok(RustType::Bool),
            TypeInner::UInt(uint_ty) => match uint_ty {
                UIntType::U1 => Ok(RustType::U1),
                UIntType::U2 => Ok(RustType::U2),
                UIntType::U4 => Ok(RustType::U4),
                UIntType::U8 => Ok(RustType::U8),
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
            TypeInner::List(element, size) => {
                let element_ty = Self::from_resolved_type(element)?;
                Ok(RustType::List(Box::new(element_ty), size.get()))
            }
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
            RustType::U1 => quote! { u8 },
            RustType::U2 => quote! { u8 },
            RustType::U4 => quote! { u8 },
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
            RustType::List(element, _size) => {
                let element_ty = element.to_type_token_stream();
                quote! { Vec<#element_ty> }
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
            RustType::U1 => {
                quote! { Value::from(UIntValue::u1(#deref #value_expr).map_err(|_e| format!("Failed to create U1 type, got: '{}', [value size in bits: '{}']", #value_expr.checked_ilog2().unwrap_or_default(), #value_expr)).unwrap()) }
            }
            RustType::U2 => {
                quote! { Value::from(UIntValue::u2(#deref #value_expr).map_err(|_e| format!("Failed to create U2 type, got: '{}', [value size in bits: '{}']", #value_expr.checked_ilog2().unwrap_or_default(), #value_expr)).unwrap()) }
            }
            RustType::U4 => {
                quote! { Value::from(UIntValue::u4(#deref #value_expr).map_err(|_e| format!("Failed to create U4 type, got: '{}', [value size in bits: '{}']", #value_expr.checked_ilog2().unwrap_or_default(), #value_expr)).unwrap()) }
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
            RustType::List(element, size) => {
                let iter_tmp_var_name = quote! { x };
                let element_conversion = {
                    element.generate_to_simplicity_conversion_inner(&iter_tmp_var_name, Some(RustTypeContext::List))
                };
                let elem_ty_generation = element.generate_simplicity_type_construction();

                quote! {
                    {
                        let elements = #value_expr.iter().map(|& #iter_tmp_var_name| #element_conversion).collect::<Vec<_>>();
                        let non_zero_pow2_size = NonZeroPow2Usize::new(#size).ok_or_else(|| format!("Failed to create non zero pow2 length, got size: '{}'", #size)).unwrap();

                        assert!(elements.len() < non_zero_pow2_size.get(), "There must be fewer list elements than the bound '{}'", non_zero_pow2_size.get());

                        Value::list(elements, #elem_ty_generation, non_zero_pow2_size)
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
            RustType::U1 => {
                quote! { ResolvedType::u1() }
            }
            RustType::U2 => {
                quote! { ResolvedType::u2() }
            }
            RustType::U4 => {
                quote! { ResolvedType::u4() }
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
            RustType::List(element, size) => {
                let elem_ty = element.generate_simplicity_type_construction();
                quote! { ResolvedType::list(#elem_ty, NonZeroPow2Usize::new(#size).ok_or_else(|| format!("Failed to create non zero pow2 length, got size: '{}'", #size)).unwrap()) }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn generate_from_value_extraction(
        &self,
        args_expr: &proc_macro2::Ident,
        witness_name: &str,
    ) -> proc_macro2::TokenStream {
        let initial_arg_name = quote! { value };
        let get_witness_expr_tokens = quote! {
            let witness_name = WitnessName::from_str_unchecked(#witness_name);
            let #initial_arg_name = #args_expr
                .get(&witness_name)
                .ok_or_else(|| format!("Missing witness: {}", #witness_name))?;
        };
        let expand_value_extraction =
            self.generate_value_extraction_from_expr(&initial_arg_name, RustTypeContext::Root);

        quote! {
            {
                #get_witness_expr_tokens
                #expand_value_extraction
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
            RustType::U1 => quote! {
                match #value_expr.inner() {
                    simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U1(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U1", #context)),
                }
            },
            RustType::U2 => quote! {
                match #value_expr.inner() {
                    simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U2(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U2", #context)),
                }
            },
            RustType::U4 => quote! {
                match #value_expr.inner() {
                    simplex::simplicityhl::value::ValueInner::UInt(UIntValue::U4(v)) => *v,
                    _ => return Err(format!("Wrong type for {}: expected U4", #context)),
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
            RustType::List(element, _size) => {
                let iter_index = quote! { i };
                let list_name = quote! { list_value };
                let elem_extraction = element
                    .generate_value_extraction_from_expr(&quote! { #list_name[#iter_index] }, RustTypeContext::List);

                quote! {
                    match #value_expr.inner() {
                        simplex::simplicityhl::value::ValueInner::List(#list_name, non_zero_pow2_size) => {
                            let list_len = #list_name.len();

                            if list_len >= non_zero_pow2_size.get() {
                                return Err(format!("Wrong list length for {}: expected less than {}, got {}", #context, non_zero_pow2_size.get(), list_len));
                            }

                            let mut res = Vec::with_capacity(list_len);

                            for #iter_index in 0..list_len {
                                res.push(#elem_extraction);
                            }

                            res
                        }
                        _ => return Err(format!("Wrong type for {}: expected List", #context)),
                    }
                }
            }
        }
    }
}
