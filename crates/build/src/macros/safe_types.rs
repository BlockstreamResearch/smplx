use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use simplicityhl::{
    ResolvedType,
    types::{BuiltinAlias, TypeInner},
};

use super::types::RustType;

pub enum SafeType {
    Alias(BuiltinAlias),
    Concrete(RustType),
    Tuple(Vec<Self>),
    Array(Box<Self>, usize),
    List(Box<Self>),
    Option(Box<Self>),
    Either(Box<Self>, Box<Self>),
}

impl SafeType {
    pub fn contains_alias(&self) -> bool {
        match self {
            Self::Alias(_) => true,
            Self::Concrete(_) => false,
            Self::Tuple(elements) => elements.iter().any(Self::contains_alias),
            Self::Array(element, _) | Self::List(element) | Self::Option(element) => element.contains_alias(),
            Self::Either(left, right) => left.contains_alias() || right.contains_alias(),
        }
    }

    pub fn from_resolved_type(ty: &ResolvedType) -> syn::Result<Self> {
        if let Some(alias) = ty.builtin_alias() {
            return Ok(Self::Alias(alias));
        }

        Ok(match ty.as_inner() {
            TypeInner::Tuple(elements) => Self::Tuple(
                elements
                    .iter()
                    .map(|ty| Self::from_resolved_type(ty))
                    .collect::<syn::Result<_>>()?,
            ),
            TypeInner::Array(element, size) => Self::Array(Box::new(Self::from_resolved_type(element)?), *size),
            TypeInner::List(element, _) => Self::List(Box::new(Self::from_resolved_type(element)?)),
            TypeInner::Option(inner) => Self::Option(Box::new(Self::from_resolved_type(inner)?)),
            TypeInner::Either(left, right) => Self::Either(
                Box::new(Self::from_resolved_type(left)?),
                Box::new(Self::from_resolved_type(right)?),
            ),
            _ => Self::Concrete(RustType::from_resolved_type(ty)?),
        })
    }

    pub fn to_type_token_stream(&self) -> TokenStream {
        match self {
            Self::Alias(alias) => {
                let name = format_ident!("{alias}");
                quote! { simplex::alias::#name }
            }
            Self::Concrete(ty) => ty.to_type_token_stream(),
            Self::Tuple(elements) => {
                let elements = elements.iter().map(Self::to_type_token_stream);
                quote! { (#(#elements,)*) }
            }
            Self::Array(element, size) => {
                let element = element.to_type_token_stream();
                quote! { [#element; #size] }
            }
            Self::List(element) => {
                let element = element.to_type_token_stream();
                quote! { Vec<#element> }
            }
            Self::Option(inner) => {
                let inner = inner.to_type_token_stream();
                quote! { Option<#inner> }
            }
            Self::Either(left, right) => {
                let left = left.to_type_token_stream();
                let right = right.to_type_token_stream();
                quote! { simplex::either::Either<#left, #right> }
            }
        }
    }

    pub fn get_default_value(&self) -> TokenStream {
        match self {
            Self::Alias(_) => {
                let ty = self.to_type_token_stream();
                quote! { <#ty as core::default::Default>::default() }
            }
            Self::Concrete(ty) => ty.get_default_value(),
            Self::Tuple(elements) => {
                let elements = elements.iter().map(Self::get_default_value);
                quote! { (#(#elements,)*) }
            }
            Self::Array(element, _) => {
                let element = element.get_default_value();
                quote! { core::array::from_fn(|_| #element) }
            }
            Self::List(_) | Self::Option(_) => quote! { core::default::Default::default() },
            Self::Either(left, _) => {
                let left = left.get_default_value();
                quote! { simplex::either::Either::Left(#left) }
            }
        }
    }

    pub fn to_concrete_value(&self, value: &TokenStream) -> TokenStream {
        match self {
            Self::Alias(_) => quote! { simplex::alias::SimplicityEncode::to_concrete_type(#value) },
            Self::Concrete(_) => quote! { *#value },
            Self::Tuple(elements) => {
                let elements = elements.iter().enumerate().map(|(index, element)| {
                    let index = syn::Index::from(index);
                    element.to_concrete_value(&quote! { &(#value).#index })
                });
                quote! { (#(#elements,)*) }
            }
            Self::Array(element, _) => {
                let conversion = element.to_concrete_value(&quote! { element });
                quote! { (#value).each_ref().map(|element| #conversion) }
            }
            Self::List(element) => {
                let conversion = element.to_concrete_value(&quote! { element });
                quote! { (#value).iter().map(|element| #conversion).collect() }
            }
            Self::Option(inner) => {
                let conversion = inner.to_concrete_value(&quote! { inner });
                quote! { (#value).as_ref().map(|inner| #conversion) }
            }
            Self::Either(left, right) => {
                let left = left.to_concrete_value(&quote! { left });
                let right = right.to_concrete_value(&quote! { right });
                quote! {
                    match #value {
                        simplex::either::Either::Left(left) => simplex::either::Either::Left(#left),
                        simplex::either::Either::Right(right) => simplex::either::Either::Right(#right),
                    }
                }
            }
        }
    }
}
