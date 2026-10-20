use std::fmt::{Display, Formatter, Result as FmtResult};

use simplicityhl::either::Either;
use simplicityhl::types::TypeInner;
use simplicityhl::value::{ValueConstructible, ValueInner};
use simplicityhl::{ResolvedType, Value};

/// A path into a witness value in the SDK's format, e.g. `["Right", "Left", "0"]`:
/// `Either` branches, tuple or array indices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct WitnessPath(Vec<String>);

static EMPTY: WitnessPath = WitnessPath(Vec::new());

impl WitnessPath {
    pub(crate) fn new<I>(routes: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        Self(routes.into_iter().map(|route| route.as_ref().to_string()).collect())
    }

    pub(crate) fn empty() -> &'static Self {
        &EMPTY
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn join(&self, route: impl Into<String>) -> Self {
        let mut routes = self.0.clone();
        routes.push(route.into());

        Self(routes)
    }

    /// Whether the path ends at an `Either` branch.
    pub(crate) fn ends_at_branch(&self) -> bool {
        matches!(self.0.last().map(String::as_str), Some("Left" | "Right"))
    }

    /// Whether every route exists in `ty`.
    pub(crate) fn fits(&self, ty: &ResolvedType) -> bool {
        Self::fits_routes(ty, &self.0)
    }

    /// Whether `value` takes the branches the path names.
    pub(crate) fn follows(&self, value: &Value) -> bool {
        Self::follows_routes(value, &self.0)
    }

    /// Draws a value of `ty` that takes the branches the path names, drawing everything off the path with `draw`.
    pub(crate) fn draw(&self, ty: &ResolvedType, draw: &mut impl FnMut(&ResolvedType) -> Value) -> Value {
        Self::draw_routes(ty, &self.0, draw)
    }

    /// `value` with the value at the path replaced by `new`.
    ///
    /// # Panics
    /// Panics when `value` does not take the branches the path names.
    pub(crate) fn replace(&self, value: &Value, new: Value) -> Value {
        Self::replace_routes(value, &self.0, new)
    }

    fn fits_routes(ty: &ResolvedType, routes: &[String]) -> bool {
        let Some((route, routes)) = routes.split_first() else {
            return true;
        };

        match (route.as_str(), ty.as_inner()) {
            ("Left", TypeInner::Either(left, _)) => Self::fits_routes(left, routes),
            ("Right", TypeInner::Either(_, right)) => Self::fits_routes(right, routes),
            (index, TypeInner::Tuple(elements)) => index
                .parse::<usize>()
                .ok()
                .and_then(|index| elements.get(index))
                .is_some_and(|element| Self::fits_routes(element, routes)),
            (index, TypeInner::Array(element, size)) => {
                index.parse::<usize>().is_ok_and(|index| index < *size) && Self::fits_routes(element, routes)
            }
            _ => false,
        }
    }

    fn follows_routes(value: &Value, routes: &[String]) -> bool {
        let Some((route, routes)) = routes.split_first() else {
            return true;
        };

        match (route.as_str(), value.inner()) {
            ("Left", ValueInner::Either(Either::Left(inner))) | ("Right", ValueInner::Either(Either::Right(inner))) => {
                Self::follows_routes(inner, routes)
            }
            (index, ValueInner::Tuple(elements) | ValueInner::Array(elements)) => index
                .parse::<usize>()
                .ok()
                .and_then(|index| elements.get(index))
                .is_some_and(|element| Self::follows_routes(element, routes)),
            _ => false,
        }
    }

    fn draw_routes(ty: &ResolvedType, routes: &[String], draw: &mut impl FnMut(&ResolvedType) -> Value) -> Value {
        let Some((route, routes)) = routes.split_first() else {
            return draw(ty);
        };

        match (route.as_str(), ty.as_inner()) {
            ("Left", TypeInner::Either(left, right)) => {
                Value::left(Self::draw_routes(left, routes, draw), (**right).clone())
            }
            ("Right", TypeInner::Either(left, right)) => {
                Value::right((**left).clone(), Self::draw_routes(right, routes, draw))
            }
            (index, TypeInner::Tuple(elements)) => match index.parse::<usize>() {
                Ok(index) if index < elements.len() => Value::tuple(
                    elements
                        .iter()
                        .enumerate()
                        .map(|(at, element)| {
                            if at == index {
                                Self::draw_routes(element, routes, draw)
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
                                Self::draw_routes(element, routes, draw)
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

    fn replace_routes(value: &Value, routes: &[String], new: Value) -> Value {
        let Some((route, rest)) = routes.split_first() else {
            return new;
        };

        match (route.as_str(), value.inner(), value.ty().as_inner()) {
            ("Left", ValueInner::Either(Either::Left(inner)), TypeInner::Either(_, right)) => {
                Value::left(Self::replace_routes(inner, rest, new), (**right).clone())
            }
            ("Right", ValueInner::Either(Either::Right(inner)), TypeInner::Either(left, _)) => {
                Value::right((**left).clone(), Self::replace_routes(inner, rest, new))
            }
            (index, ValueInner::Tuple(elements), _) => {
                let index = index.parse::<usize>().expect("a tuple route is an index");

                Value::tuple(elements.iter().enumerate().map(|(at, element)| {
                    if at == index {
                        Self::replace_routes(element, rest, new.clone())
                    } else {
                        element.clone()
                    }
                }))
            }
            _ => panic!("{value} does not take the path {routes:?}"),
        }
    }
}

impl Display for WitnessPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:?}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use simplicityhl::types::TypeConstructible;

    use crate::fuzz::utils::generate_random_value_by_ty;

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
        let path = WitnessPath::new(["Right", "Left", "0"]);
        let mut rng = rand::rng();

        for _ in 0..64 {
            let value = path.draw(&ty, &mut |ty| generate_random_value_by_ty(ty, &mut rng));

            assert!(path.follows(&value), "{value} is not on the path");
            assert!(value.is_of_type(&ty));
        }
    }
}
