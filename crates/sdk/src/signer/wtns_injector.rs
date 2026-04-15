use std::sync::Arc;

use simplicityhl::{
    ResolvedType, Value,
    types::TypeInner,
    value::{ValueConstructible, ValueInner},
};

use crate::signer::error::WtnsWrappingError;

/// Struct for injecting specific value by given path into witness value
#[derive(Clone)]
pub struct WtnsInjector {
    path: Vec<WtnsPathRoute>,
}

impl WtnsInjector {
    /// ## Usage
    /// ```rust,ignore
    /// // .simf script
    /// match witness::SOMETHING {
    ///     Left(x: u64) => ...,
    ///     Right([y, z]: [u64, u64]) => ...
    /// }
    /// // path for each variable
    /// vec!["Left"] // for x
    /// vec!["Right", "0"] // for y
    /// vec!["Right", "1"] // for z
    /// ```
    pub fn new(path: &[String]) -> Result<Self, WtnsWrappingError> {
        let parsed_path = path
            .iter()
            .map(|route| match route.as_str() {
                "Left" => Ok(WtnsPathRoute::Either(EitherRoute::Left)),
                "Right" => Ok(WtnsPathRoute::Either(EitherRoute::Right)),
                s => s
                    .parse::<usize>()
                    .map(|n| WtnsPathRoute::Enumerable(EnumerableRoute(n)))
                    .map_err(|_| WtnsWrappingError::ParsingError),
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { path: parsed_path })
    }

    /// Constructs new value by intjecting given value into witness at the position described by `path`.
    /// Consistency between `witness` and `witness_types` should be guaranteed by caller.
    pub fn inject_value(
        &self,
        witness: &Arc<Value>,
        witness_types: &ResolvedType,
        value: Value,
    ) -> Result<Value, WtnsWrappingError> {
        enum StackItem {
            Either(EitherRoute, Arc<ResolvedType>),
            Array(EnumerableRoute, Arc<ResolvedType>, Arc<[Value]>),
            Tuple(EnumerableRoute, Arc<[Value]>),
        }

        // invocations of these functions below determined from types during traversal
        // matches! guard at top of loop guarantees that types and routes are consistent
        fn downcast_either(val: &Value, direction: EitherRoute) -> Arc<Value> {
            match (direction, val.inner()) {
                (EitherRoute::Left, ValueInner::Either(either)) => Arc::clone(either.as_ref().unwrap_left()),
                (EitherRoute::Right, ValueInner::Either(either)) => Arc::clone(either.as_ref().unwrap_right()),
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

        let mut stack = Vec::new();
        let mut current_val = Arc::clone(witness);
        let mut current_ty = witness_types;

        for route in self.path.iter() {
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
                    match direction {
                        EitherRoute::Left => {
                            stack.push(StackItem::Either(direction, Arc::clone(right_ty)));
                            current_ty = left_ty;
                        }
                        EitherRoute::Right => {
                            stack.push(StackItem::Either(direction, Arc::clone(left_ty)));
                            current_ty = right_ty;
                        }
                    }
                    current_val = downcast_either(&current_val, direction);
                }
                TypeInner::Array(ty, len) => {
                    let idx: EnumerableRoute = (*route).try_into().expect("Checked in matches! above");

                    if idx.0 >= *len {
                        return Err(WtnsWrappingError::IdxOutOfBounds(*len, idx.0));
                    }

                    let arr_val = downcast_array(&current_val);

                    stack.push(StackItem::Array(idx, Arc::clone(ty), Arc::clone(&arr_val)));

                    current_ty = ty;
                    current_val = Arc::new(arr_val[idx.0].clone());
                }
                TypeInner::Tuple(tuple) => {
                    let idx: EnumerableRoute = (*route).try_into().expect("Checked in matches! above");

                    if idx.0 >= tuple.len() {
                        return Err(WtnsWrappingError::IdxOutOfBounds(tuple.len(), idx.0));
                    }

                    let tuple_val = downcast_tuple(&current_val);

                    stack.push(StackItem::Tuple(idx, Arc::clone(&tuple_val)));

                    current_ty = &tuple[idx.0];
                    current_val = Arc::new(tuple_val[idx.0].clone());
                }
                _ => unreachable!("checked at the top of loop"),
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
}

#[derive(Clone, Copy, Debug)]
pub enum WtnsPathRoute {
    Either(EitherRoute),
    Enumerable(EnumerableRoute),
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

#[derive(Clone, Copy, Debug)]
pub enum EitherRoute {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug)]
pub struct EnumerableRoute(usize);
