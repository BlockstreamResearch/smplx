use std::sync::Arc;

use simplicityhl::{
    ResolvedType, Value,
    either::Either,
    types::TypeInner,
    value::{ValueConstructible, ValueInner},
};

use crate::signer::error::WtnsWrappingError;

#[derive(Clone, Copy, Debug)]
pub struct EnumerableRoute(usize);

#[derive(Clone, Copy, Debug)]
pub enum EitherRoute {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug)]
pub enum WtnsPathRoute {
    Either(EitherRoute),
    Enumerable(EnumerableRoute),
}

#[derive(Clone)]
pub struct WtnsInjector {}

enum StackItem {
    Either(EitherRoute, Arc<ResolvedType>),
    Array(EnumerableRoute, Arc<ResolvedType>, Arc<[Value]>),
    Tuple(EnumerableRoute, Arc<[Value]>),
}

impl WtnsInjector {
    /// Constructs new value by injecting given value into witness at the position described by `path`.
    /// Consistency between `witness` and `witness_types` should be guaranteed by the caller.
    pub fn inject_value<I>(
        witness: &Arc<Value>,
        witness_types: &ResolvedType,
        path: I,
        value: Value,
    ) -> Result<Value, WtnsWrappingError>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let parsed_path = Self::parse_path(path)?;

        let mut stack = Vec::new();
        let mut current_val = Arc::clone(witness);
        let mut current_ty = witness_types;

        for route in parsed_path.iter() {
            if !matches!(
                (route, current_ty.as_inner()),
                (WtnsPathRoute::Enumerable(_), TypeInner::Array(_, _))
                    | (WtnsPathRoute::Enumerable(_), TypeInner::Tuple(_))
                    | (WtnsPathRoute::Either(_), TypeInner::Either(_, _))
            ) {
                return Err(WtnsWrappingError::UnsupportedPathType(current_ty.to_string()));
            }

            match current_ty.as_inner() {
                TypeInner::Either(left_ty, right_ty) => {
                    let direction: EitherRoute = (*route).try_into().expect("Checked in matches! above");
                    let either_val = Self::downcast_either(&current_val);

                    match (direction, either_val.is_right()) {
                        (EitherRoute::Left, false) => {
                            stack.push(StackItem::Either(direction, Arc::clone(right_ty)));
                            current_ty = left_ty;
                            current_val = Arc::clone(either_val.as_ref().unwrap_left())
                        }
                        (EitherRoute::Right, true) => {
                            stack.push(StackItem::Either(direction, Arc::clone(left_ty)));
                            current_ty = right_ty;
                            current_val = Arc::clone(either_val.as_ref().unwrap_right())
                        }
                        _ => return Err(WtnsWrappingError::EitherBranchMismatch),
                    }
                }
                TypeInner::Array(ty, len) => {
                    let idx: EnumerableRoute = (*route).try_into().expect("Checked in matches! above");

                    if idx.0 >= *len {
                        return Err(WtnsWrappingError::IdxOutOfBounds(*len, idx.0));
                    }

                    let arr_val = Self::downcast_array(&current_val);

                    stack.push(StackItem::Array(idx, Arc::clone(ty), Arc::clone(&arr_val)));

                    current_ty = ty;
                    current_val = Arc::new(arr_val[idx.0].clone());
                }
                TypeInner::Tuple(tuple) => {
                    let idx: EnumerableRoute = (*route).try_into().expect("Checked in matches! above");

                    if idx.0 >= tuple.len() {
                        return Err(WtnsWrappingError::IdxOutOfBounds(tuple.len(), idx.0));
                    }

                    let tuple_val = Self::downcast_tuple(&current_val);

                    stack.push(StackItem::Tuple(idx, Arc::clone(&tuple_val)));

                    current_ty = &tuple[idx.0];
                    current_val = Arc::new(tuple_val[idx.0].clone());
                }
                _ => unreachable!("checked at the top of the loop"),
            }
        }

        if value.ty() != current_ty {
            return Err(WtnsWrappingError::RootTypeMismatch(
                current_ty.to_string(),
                value.ty().to_string(),
            ));
        }

        let mut value = value;

        for item in stack.into_iter().rev() {
            value = match item {
                StackItem::Either(direction, sibling_ty) => match direction {
                    EitherRoute::Left => Value::left(value, (*sibling_ty).clone()),
                    EitherRoute::Right => Value::right((*sibling_ty).clone(), value),
                },
                StackItem::Array(idx, elem_ty, arr) => {
                    let mut elements = arr.to_vec();
                    elements[idx.0] = value;
                    Value::array(elements, (*elem_ty).clone())
                }
                StackItem::Tuple(idx, tuple_vals) => {
                    let mut elements = tuple_vals.to_vec();
                    elements[idx.0] = value;
                    Value::tuple(elements)
                }
            };
        }

        Ok(value)
    }

    fn parse_path<I>(path: I) -> Result<Vec<WtnsPathRoute>, WtnsWrappingError>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        path.into_iter()
            .map(|route| match route.as_ref() {
                "Left" => Ok(WtnsPathRoute::Either(EitherRoute::Left)),
                "Right" => Ok(WtnsPathRoute::Either(EitherRoute::Right)),
                s => s
                    .parse::<usize>()
                    .map(|n| WtnsPathRoute::Enumerable(EnumerableRoute(n)))
                    .map_err(|_| WtnsWrappingError::ParsingError),
            })
            .collect::<Result<Vec<_>, _>>()
    }

    fn downcast_either(val: &Value) -> &Either<Arc<Value>, Arc<Value>> {
        match val.inner() {
            ValueInner::Either(either) => either,
            _ => unreachable!(),
        }
    }

    fn downcast_array(val: &Value) -> Arc<[Value]> {
        match val.inner() {
            ValueInner::Array(arr) => Arc::clone(arr),
            _ => unreachable!(),
        }
    }

    fn downcast_tuple(val: &Value) -> Arc<[Value]> {
        match val.inner() {
            ValueInner::Tuple(arr) => Arc::clone(arr),
            _ => unreachable!(),
        }
    }
}

impl TryInto<EitherRoute> for WtnsPathRoute {
    type Error = WtnsPathRoute;

    fn try_into(self) -> Result<EitherRoute, Self::Error> {
        match self {
            Self::Either(direction) => Ok(direction),
            _ => Err(self),
        }
    }
}

impl TryInto<EnumerableRoute> for WtnsPathRoute {
    type Error = WtnsPathRoute;

    fn try_into(self) -> Result<EnumerableRoute, Self::Error> {
        match self {
            Self::Enumerable(tuple) => Ok(tuple),
            _ => Err(self),
        }
    }
}

#[cfg(test)]
mod test {
    use simplicityhl::types::TypeConstructible;

    use super::*;

    fn dummy_value() -> Value {
        // Either<(u64, Either<u64, u64>), [u8; 4]>
        Value::left(
            Value::tuple([Value::u64(0), Value::right(ResolvedType::u64(), Value::u64(1))]),
            ResolvedType::array(ResolvedType::u8(), 64),
        )
    }

    #[test]
    fn inject_value_success() {
        let witness = Arc::new(dummy_value());
        let witness_types = witness.ty();

        let injected_val_tuple =
            WtnsInjector::inject_value(&witness, witness_types, &["Left", "0"], Value::u64(3)).unwrap();

        assert_eq!(
            injected_val_tuple,
            Value::parse_from_str("Left((3, Right(1)))", witness_types).unwrap()
        );

        let injected_val_either = WtnsInjector::inject_value(
            &witness,
            witness_types,
            &["Left", "1"],
            Value::left(Value::u64(2), ResolvedType::u64()),
        )
        .unwrap();

        assert_eq!(
            injected_val_either,
            Value::parse_from_str("Left((0, Left(2)))", witness_types).unwrap()
        );
    }

    #[test]
    fn inject_value_idx_out_of_bounds() {
        let witness = Arc::new(dummy_value());
        let witness_types = witness.ty();

        let err = WtnsInjector::inject_value(&witness, witness_types, &["Left", "5"], Value::u64(0)).unwrap_err();

        assert!(matches!(err, WtnsWrappingError::IdxOutOfBounds(_, _)));
    }

    #[test]
    fn inject_value_root_mismatch() {
        let witness = Arc::new(dummy_value());
        let witness_types = witness.ty();

        let err = WtnsInjector::inject_value(&witness, witness_types, &["Left", "1"], Value::unit()).unwrap_err();

        assert!(matches!(err, WtnsWrappingError::RootTypeMismatch(_, _)));
    }

    #[test]
    fn inject_value_either_branch_mismatch() {
        let witness = Arc::new(dummy_value());
        let witness_types = witness.ty();

        let err = WtnsInjector::inject_value(
            &witness,
            witness_types,
            &["Right"],
            Value::right(
                ResolvedType::tuple([
                    ResolvedType::u64(),
                    ResolvedType::either(ResolvedType::u64(), ResolvedType::u64()),
                ]),
                Value::array(vec![Value::u8(0)], ResolvedType::u8()),
            ),
        )
        .unwrap_err();

        assert!(matches!(err, WtnsWrappingError::EitherBranchMismatch));
    }
}
