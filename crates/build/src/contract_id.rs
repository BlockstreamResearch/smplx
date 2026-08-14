use std::collections::HashMap;

use serde::{Serialize, Serializer};

use simplicityhl::num::U256;
use simplicityhl::simplicity::Cmr;
use simplicityhl::types::{TypeInner, UIntType};
use simplicityhl::value::{UIntValue, ValueConstructible};
use simplicityhl::{Arguments, Parameters, ResolvedType, TemplateProgram, Value};

use crate::error::BuildError;

/// The CMR of a contract compiled with default arguments.
pub(crate) struct ContractId(Cmr);

impl ContractId {
    /// # Errors
    /// Returns a `BuildError` if a parameter type is unsupported, or if the program does not compile.
    pub(crate) fn dry_run(template: &TemplateProgram) -> Result<Self, BuildError> {
        let arguments = Self::default_arguments(template.parameters())?;
        let compiled = template.instantiate(arguments, false).map_err(BuildError::DryRun)?;

        Ok(Self(compiled.commit().cmr()))
    }

    /// Real arguments live in user code and do not exist at build time,
    /// so the defaults stand in for them.
    fn default_arguments(parameters: &Parameters) -> Result<Arguments, BuildError> {
        parameters
            .iter()
            .map(|(name, ty)| Ok((name.clone(), Self::default_value(ty)?)))
            .collect::<Result<HashMap<_, _>, _>>()
            .map(Arguments::from)
    }

    fn default_value(ty: &ResolvedType) -> Result<Value, BuildError> {
        match ty.as_inner() {
            TypeInner::Boolean => Ok(Value::from(false)),
            TypeInner::UInt(uint_ty) => Ok(Value::from(Self::default_uint(*uint_ty))),
            TypeInner::Option(inner) => Ok(Value::none(inner.as_ref().clone())),
            TypeInner::Either(left, right) => Ok(Value::left(Self::default_value(left)?, right.as_ref().clone())),
            TypeInner::Tuple(elements) => {
                let elements = elements
                    .iter()
                    .map(|element| Self::default_value(element))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Value::tuple(elements))
            }
            TypeInner::Array(element, size) => {
                let elements = (0..*size)
                    .map(|_| Self::default_value(element))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Value::array(elements, element.as_ref().clone()))
            }
            TypeInner::List(element, bound) => Ok(Value::list(std::iter::empty(), element.as_ref().clone(), *bound)),
            _ => Err(BuildError::DryRun(format!(
                "Cannot build a default argument for unsupported type '{ty}'"
            ))),
        }
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
}

/// Flattens to a bare hex string instead of an object.
impl Serialize for ContractId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}
