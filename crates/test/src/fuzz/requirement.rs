use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result as FmtResult};

use simplicityhl::either::Either;
use simplicityhl::value::ValueInner;
use simplicityhl::{TemplateProgramWitness, Value, WitnessNameToValueMap, WitnessValues};

use smplx_sdk::transaction::FinalTransaction;

use crate::fuzz::mutations::UtxoMap;
use crate::fuzz::program_scenario::FuzzProgramScenario;
use crate::fuzz::utils::replace_on_path;

/// The witness values the requirements of a case set.
pub(crate) struct Requirements<'a> {
    values: Vec<RequiredValue<'a>>,
}

/// A witness value a requirement sets, with the generated value it replaces.
pub(crate) struct RequiredValue<'a> {
    program_scenario: &'a FuzzProgramScenario,
    field: TemplateProgramWitness,
    path: Vec<String>,
    required: Value,
    generated: Value,
}

impl<'a> Requirements<'a> {
    pub(crate) fn new() -> Self {
        Self { values: Vec::new() }
    }

    /// Records every value of `required` that differs from `generated`, down to `Either` branches and tuple elements.
    pub(crate) fn record_diff(
        &mut self,
        program_scenario: &'a FuzzProgramScenario,
        generated: &WitnessValues,
        required: &WitnessValues,
    ) {
        let mut fields: Vec<_> = required.iter().collect();
        fields.sort_by_key(|(field, _)| *field);

        for (field, required) in fields {
            if let Some(generated) = generated.get(field) {
                self.record_value(RequiredValue {
                    program_scenario,
                    field: field.clone(),
                    path: Vec::new(),
                    required: required.clone(),
                    generated: generated.clone(),
                });
            }
        }
    }

    /// Puts the generated value back into `transaction` for the required value `pick` chooses.
    pub(crate) fn violate_one(
        mut self,
        pick: Option<u64>,
        transaction: &mut FinalTransaction,
        map: &UtxoMap,
    ) -> Option<RequiredValue<'a>> {
        let pick = pick.filter(|_| !self.values.is_empty())?;
        let violated = self.values.swap_remove((pick % self.values.len() as u64) as usize);

        let witness = violated
            .program_scenario
            .witness_in(transaction, map)
            .expect("a required value is in the witness of a spent input");
        let mut fields: HashMap<_, _> = witness
            .iter()
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect();
        let value = replace_on_path(&fields[&violated.field], &violated.path, violated.generated.clone());

        fields.insert(violated.field.clone(), value);
        violated
            .program_scenario
            .place_witness(transaction, map, WitnessValues::from_map(fields));

        Some(violated)
    }

    fn record_value(&mut self, value: RequiredValue<'a>) {
        let routes: Vec<(String, Value, Value)> = match (value.generated.inner(), value.required.inner()) {
            (ValueInner::Either(Either::Left(generated)), ValueInner::Either(Either::Left(required))) => {
                vec![("Left".to_string(), (**generated).clone(), (**required).clone())]
            }
            (ValueInner::Either(Either::Right(generated)), ValueInner::Either(Either::Right(required))) => {
                vec![("Right".to_string(), (**generated).clone(), (**required).clone())]
            }
            (ValueInner::Tuple(generated), ValueInner::Tuple(required)) => generated
                .iter()
                .zip(required.iter())
                .enumerate()
                .map(|(index, (generated, required))| (index.to_string(), generated.clone(), required.clone()))
                .collect(),
            _ => {
                if value.generated != value.required {
                    self.values.push(value);
                }

                return;
            }
        };

        for (route, generated, required) in routes {
            let mut path = value.path.clone();
            path.push(route);

            self.record_value(RequiredValue {
                program_scenario: value.program_scenario,
                field: value.field.clone(),
                path,
                required,
                generated,
            });
        }
    }
}

impl Display for RequiredValue<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "`{}` witness `{}`", self.program_scenario.name, self.field)?;

        if !self.path.is_empty() {
            write!(f, " at {:?}", self.path)?;
        }

        write!(f, " is {} instead of {}", self.generated, self.required)
    }
}
