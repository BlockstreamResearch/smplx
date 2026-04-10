use std::sync::Arc;

use simplicityhl::{
    ResolvedType, Value,
    types::TypeInner,
    value::{ValueConstructible, ValueInner},
};

use crate::signer::SignerError;

#[derive(Clone, Copy, Debug)]
pub enum WtnsPathRoute {
    Either(EitherRoute),
    Tuple(EnumerableRoute),
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
            Self::Tuple(tuple) => Ok(tuple),
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

pub fn parse_sig_path(path: &Vec<String>) -> Result<Vec<WtnsPathRoute>, SignerError> {
    let mut res = Vec::new();

    for route in path {
        let parsed = match route.as_str() {
            "Left" => WtnsPathRoute::Either(EitherRoute::Left),
            "Right" => WtnsPathRoute::Either(EitherRoute::Right),
            _ if route.parse::<usize>().is_ok() => {
                WtnsPathRoute::Tuple(EnumerableRoute(route.parse::<usize>().unwrap()))
            }
            _ => return Err(SignerError::WtnsSigParse),
        };
        res.push(parsed);
    }
    Ok(res)
}

pub enum WtnsWrappingError {
    UnsupportedPathType,
    TupleOutOfBounds(usize, usize),
    RootTypeMismatch,
}

impl std::fmt::Display for WtnsWrappingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootTypeMismatch => write!(f, "injected value's type does not match with type declared in witness"),
            Self::UnsupportedPathType => write!(f, "unsupported path type; only Either, Array and Tuple are available"),
            Self::TupleOutOfBounds(expected, input) => {
                let msg = format!("index out of bound; length is {}, got {}", expected, input);
                write!(f, "{}", msg)
            }
        }
    }
}

impl From<WtnsWrappingError> for SignerError {
    fn from(value: WtnsWrappingError) -> Self {
        Self::WtnsInjectError(value.to_string())
    }
}

pub fn wrap_value_along_path(
    existing_witness: &Arc<Value>,
    ty: &ResolvedType,
    injected_val: Value,
    path: &[WtnsPathRoute],
) -> Result<Value, WtnsWrappingError> {
    enum StackItem {
        Either(EitherRoute, Arc<ResolvedType>),
        Array(EnumerableRoute, Arc<ResolvedType>, Arc<[Value]>),
        Tuple(EnumerableRoute, Arc<[Value]>),
    }

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
    let mut current_val = Arc::clone(existing_witness);
    let mut current_ty = ty;

    for route in path {
        if !matches!(
            (route, current_ty.as_inner()),
            (WtnsPathRoute::Tuple(_), TypeInner::Array(_, _))
                | (WtnsPathRoute::Tuple(_), TypeInner::Tuple(_))
                | (WtnsPathRoute::Either(_), TypeInner::Either(_, _))
        ) {
            return Err(WtnsWrappingError::UnsupportedPathType);
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
                    return Err(WtnsWrappingError::TupleOutOfBounds(*len, idx.0));
                }

                let arr_val = downcast_array(&current_val);

                stack.push(StackItem::Array(idx, Arc::clone(ty), Arc::clone(&arr_val)));

                current_ty = ty;
                current_val = Arc::new(arr_val[idx.0].clone());
            }
            TypeInner::Tuple(tuple) => {
                let idx: EnumerableRoute = (*route).try_into().expect("Checked in matches! above");

                if idx.0 >= tuple.len() {
                    return Err(WtnsWrappingError::TupleOutOfBounds(tuple.len(), idx.0));
                }

                let tuple_val = downcast_tuple(&current_val);

                stack.push(StackItem::Tuple(idx, Arc::clone(&tuple_val)));

                current_ty = &tuple[idx.0];
                current_val = Arc::new(tuple_val[idx.0].clone());
            }
            _ => return Err(WtnsWrappingError::UnsupportedPathType),
        }
    }

    if injected_val.ty() != current_ty {
        return Err(WtnsWrappingError::RootTypeMismatch);
    }

    let mut value = injected_val;

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
