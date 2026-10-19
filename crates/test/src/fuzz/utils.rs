use std::collections::HashMap;

use simplicityhl::either::Either;
use simplicityhl::num::U256;
use simplicityhl::types::{TypeInner, UIntType};
use simplicityhl::value::{ValueConstructible, ValueInner};
use simplicityhl::{Arguments, ResolvedType, TemplateProgramWitness, Value, WitnessNameToValueMap, WitnessValues};

use rand::{Rng, RngExt};

use smplx_sdk::program::ProgramSchema;

pub fn random_arguments<P: ProgramSchema, R: Rng + ?Sized>(rng: &mut R) -> Arguments {
    Arguments::from_map(random_fields(&P::argument_types(), rng))
}

pub fn random_witness<P: ProgramSchema, R: Rng + ?Sized>(rng: &mut R) -> WitnessValues {
    WitnessValues::from_map(random_fields(&P::witness_types(), rng))
}

pub fn random_fields<R: Rng + ?Sized>(
    types: &[(TemplateProgramWitness, ResolvedType)],
    rng: &mut R,
) -> HashMap<TemplateProgramWitness, Value> {
    types
        .iter()
        .map(|(name, ty)| (name.clone(), generate_random_value_by_ty(ty, rng)))
        .collect()
}

pub fn generate_random_value_by_ty<R: Rng + ?Sized>(ty: &ResolvedType, rng: &mut R) -> Value {
    match ty.as_inner() {
        TypeInner::Either(left_ty, right_ty) => match rng.random::<bool>() {
            true => Value::left(generate_random_value_by_ty(left_ty, rng), (**right_ty).clone()),
            false => Value::right((**left_ty).clone(), generate_random_value_by_ty(right_ty, rng)),
        },
        TypeInner::Option(option_ty) => match rng.random::<bool>() {
            true => Value::some(generate_random_value_by_ty(option_ty, rng)),
            false => Value::none((**option_ty).clone()),
        },
        TypeInner::Boolean => Value::from(rng.random::<bool>()),
        TypeInner::UInt(x) => match x {
            UIntType::U1 => Value::u1(rng.random::<u8>() & 0x0001),
            UIntType::U2 => Value::u2(rng.random::<u8>() & 0x0003),
            UIntType::U4 => Value::u4(rng.random::<u8>() & 0x000F),
            UIntType::U8 => Value::u8(rng.random::<u8>()),
            UIntType::U16 => Value::u16(rng.random::<u16>()),
            UIntType::U32 => Value::u32(rng.random::<u32>()),
            UIntType::U64 => Value::u64(rng.random::<u64>()),
            UIntType::U128 => Value::u128(rng.random::<u128>()),
            UIntType::U256 => Value::u256(U256::from_byte_array(rng.random())),
        },
        TypeInner::Tuple(tuple_ty) => Value::tuple(tuple_ty.iter().map(|x| generate_random_value_by_ty(x, rng))),
        TypeInner::Array(array_ty, size) => Value::array(
            (0..*size).map(|_| generate_random_value_by_ty(array_ty, rng)),
            (**array_ty).clone(),
        ),
        TypeInner::List(list_ty, size_pow_2) => {
            let size = rng.random_range(0..size_pow_2.get());
            Value::list(
                (0..size).map(|_| generate_random_value_by_ty(list_ty, rng)),
                (**list_ty).clone(),
                *size_pow_2,
            )
        }
        _ => Value::unit(),
    }
}

/// Draws a value of `ty` that takes the branches `path` names, drawing everything off the path with `draw`.
pub(crate) fn draw_on_path(ty: &ResolvedType, path: &[String], draw: &mut impl FnMut(&ResolvedType) -> Value) -> Value {
    let Some((route, path)) = path.split_first() else {
        return draw(ty);
    };

    match (route.as_str(), ty.as_inner()) {
        ("Left", TypeInner::Either(left, right)) => Value::left(draw_on_path(left, path, draw), (**right).clone()),
        ("Right", TypeInner::Either(left, right)) => Value::right((**left).clone(), draw_on_path(right, path, draw)),
        (index, TypeInner::Tuple(elements)) => match index.parse::<usize>() {
            Ok(index) if index < elements.len() => Value::tuple(
                elements
                    .iter()
                    .enumerate()
                    .map(|(at, element)| {
                        if at == index {
                            draw_on_path(element, path, draw)
                        } else {
                            draw(element)
                        }
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => draw(ty),
        },
        (index, TypeInner::Array(element, size)) => match index.parse::<usize>() {
            Ok(index) if index < *size => Value::array(
                (0..*size)
                    .map(|at| {
                        if at == index {
                            draw_on_path(element, path, draw)
                        } else {
                            draw(element)
                        }
                    })
                    .collect::<Vec<_>>(),
                (**element).clone(),
            ),
            _ => draw(ty),
        },
        _ => draw(ty),
    }
}

/// Whether every route of `path` exists in `ty`.
pub(crate) fn fits_path(ty: &ResolvedType, path: &[String]) -> bool {
    let Some((route, path)) = path.split_first() else {
        return true;
    };

    match (route.as_str(), ty.as_inner()) {
        ("Left", TypeInner::Either(left, _)) => fits_path(left, path),
        ("Right", TypeInner::Either(_, right)) => fits_path(right, path),
        (index, TypeInner::Tuple(elements)) => index
            .parse::<usize>()
            .ok()
            .and_then(|index| elements.get(index))
            .is_some_and(|element| fits_path(element, path)),
        (index, TypeInner::Array(element, size)) => {
            index.parse::<usize>().is_ok_and(|index| index < *size) && fits_path(element, path)
        }
        _ => false,
    }
}

/// Whether `value` takes the branches `path` names.
pub(crate) fn follows_path(value: &Value, path: &[String]) -> bool {
    let Some((route, path)) = path.split_first() else {
        return true;
    };

    match (route.as_str(), value.inner()) {
        ("Left", ValueInner::Either(Either::Left(inner))) | ("Right", ValueInner::Either(Either::Right(inner))) => {
            follows_path(inner, path)
        }
        (index, ValueInner::Tuple(elements) | ValueInner::Array(elements)) => index
            .parse::<usize>()
            .ok()
            .and_then(|index| elements.get(index))
            .is_some_and(|element| follows_path(element, path)),
        _ => false,
    }
}

/// Replaces `value` value with `new` at `path`.
///
/// # Panics
/// Panics when `value` does not take the branches `path` names.
pub(crate) fn replace_on_path(value: &Value, path: &[String], new: Value) -> Value {
    let Some((route, path)) = path.split_first() else {
        return new;
    };

    match (route.as_str(), value.inner(), value.ty().as_inner()) {
        ("Left", ValueInner::Either(Either::Left(inner)), TypeInner::Either(_, right)) => {
            Value::left(replace_on_path(inner, path, new), (**right).clone())
        }
        ("Right", ValueInner::Either(Either::Right(inner)), TypeInner::Either(left, _)) => {
            Value::right((**left).clone(), replace_on_path(inner, path, new))
        }
        (index, ValueInner::Tuple(elements), _) => {
            let index = index.parse::<usize>().expect("a tuple route is an index");

            Value::tuple(elements.iter().enumerate().map(|(at, element)| {
                if at == index {
                    replace_on_path(element, path, new.clone())
                } else {
                    element.clone()
                }
            }))
        }
        _ => panic!("{value} does not take the path {path:?}"),
    }
}

#[cfg(test)]
mod tests {
    use simplicityhl::either::Either;
    use simplicityhl::types::TypeConstructible;
    use simplicityhl::value::ValueInner;

    use super::*;

    #[test]
    fn draws_along_the_path() {
        let ty = ResolvedType::either(
            ResolvedType::u32(),
            ResolvedType::either(
                ResolvedType::tuple([ResolvedType::u256(), ResolvedType::u32()]),
                ResolvedType::u32(),
            ),
        );
        let path = ["Right", "Left", "0"].map(String::from);
        let mut rng = rand::rng();

        for _ in 0..64 {
            let value = draw_on_path(&ty, &path, &mut |ty| generate_random_value_by_ty(ty, &mut rng));

            let ValueInner::Either(Either::Right(inner)) = value.inner() else {
                panic!("{value} is not on the path")
            };
            assert!(
                matches!(inner.inner(), ValueInner::Either(Either::Left(_))),
                "{value} is not on the path"
            );
            assert!(value.is_of_type(&ty));
        }
    }
}
