use std::any::{TypeId, type_name};
use std::collections::{HashMap, HashSet};
use std::panic::{self, AssertUnwindSafe};

use simplicityhl::WitnessNameToValueMap;

use smplx_sdk::program::Program;
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature};

use crate::fuzz::FuzzProgramTrait;
use crate::fuzz::mutations::Mutations;
use crate::fuzz::outcome::SetupError;
use crate::fuzz::program_scenario::{FuzzProgramScenario, FuzzProgramScenarioBuilder};
use crate::fuzz::program_strategy::FuzzProgramValues;
use crate::fuzz::requirement::{RequiredValue, Requirements};
use crate::fuzz::scenario_strategy::{CaseValues, CombinedStrategy};
use crate::fuzz::spec::FuzzCase;
use crate::fuzz::utils::{fits_path, follows_path};

/// A transaction and the programs being fuzzed.
pub struct Scenario {
    transaction: FinalTransaction,
    program_scenarios: Vec<FuzzProgramScenario>,
}

impl Scenario {
    /// Fuzzes the programs added to `transaction`. Everything they do not override stays as assembled.
    pub fn new(transaction: FinalTransaction) -> Self {
        Self {
            transaction,
            program_scenarios: Vec::new(),
        }
    }

    pub fn add<P: FuzzProgramTrait>(&mut self, name: impl Into<String>) -> FuzzProgramScenarioBuilder<'_, P> {
        self.program_scenarios.push(FuzzProgramScenario::new::<P>(name.into()));

        FuzzProgramScenarioBuilder::new(self.program_scenarios.last_mut().expect("just added"))
    }

    /// # Errors
    /// Returns a [`SetupError`] when `mutations` cannot mutate the transaction.
    pub(crate) fn combined_strategy(&self, mutations: Option<&Mutations>) -> Result<CombinedStrategy, SetupError> {
        CombinedStrategy::new(&self.program_scenarios, &self.transaction, mutations)
    }

    /// # Errors
    /// Returns [`SetupError::Scenario`] when the programs do not fit the transaction or each other.
    pub(crate) fn validate(&self) -> Result<(), SetupError> {
        let invalid = |reason: String| Err(SetupError::Scenario(reason));

        let mut names = HashSet::new();
        let mut spent = HashMap::new();
        let mut locked = HashMap::new();

        for program_scenario in &self.program_scenarios {
            let name = &program_scenario.name;

            if !names.insert(name) {
                return invalid(format!("two programs are named `{name}`"));
            }

            for (field, path) in &program_scenario.spending_paths {
                if !matches!(path.last().map(String::as_str), Some("Left" | "Right")) {
                    return invalid(format!(
                        "`{name}` has the spending path {path:?} in `{field}`, which must end at an `Either` branch"
                    ));
                }

                let fits = program_scenario
                    .witness_types
                    .iter()
                    .find(|(witness, _)| witness == field)
                    .is_some_and(|(_, ty)| fits_path(ty, path));

                if !fits {
                    return invalid(format!(
                        "`{name}` has no witness field `{field}` with the spending path {path:?}"
                    ));
                }
            }

            if program_scenario.inputs.is_empty()
                && !(program_scenario.witness_bindings.is_empty() && program_scenario.witness_requirements.is_empty())
            {
                return invalid(format!("`{name}` binds or requires its witness, but spends no input"));
            }

            for &index in &program_scenario.inputs {
                if index >= self.transaction.n_inputs() {
                    return invalid(format!(
                        "`{name}` spends input {index}, but the transaction has {} input(s)",
                        self.transaction.n_inputs()
                    ));
                }

                if let Some(other) = spent.insert(index, name) {
                    return invalid(format!("input {index} is spent by both `{other}` and `{name}`"));
                }
            }

            for &index in &program_scenario.outputs {
                if index >= self.transaction.n_outputs() {
                    return invalid(format!(
                        "`{name}` locks output {index}, but the transaction has {} output(s)",
                        self.transaction.n_outputs()
                    ));
                }

                if let Some(other) = locked.insert(index, name) {
                    return invalid(format!("output {index} is locked by both `{other}` and `{name}`"));
                }
            }
        }

        if spent.is_empty() {
            return invalid("no program spends an input".to_string());
        }

        Ok(())
    }

    /// Binds and builds every program in order and places it into the transaction, then binds the witnesses
    /// and sets the required values. Returns the required value the case violates, if any.
    ///
    /// # Errors
    /// Returns a [`SetupError`] when the values do not fit a program or a binding panics.
    pub(crate) fn build(
        &self,
        values: CaseValues,
        signer: &Signer,
    ) -> Result<(Vec<Program>, FinalTransaction, Option<RequiredValue<'_>>), SetupError> {
        let CaseValues {
            programs: program_values,
            change,
        } = values;
        let mutations = change.mutations();
        let mut programs = Vec::with_capacity(self.program_scenarios.len());
        let mut transaction = self.transaction.clone();

        for mutation in mutations {
            mutation.apply(&mut transaction)?;
        }

        for (program_scenario, values) in self.program_scenarios.iter().zip(program_values) {
            let FuzzProgramValues {
                arguments,
                witness,
                storage,
            } = values;
            let name = &program_scenario.name;

            if storage.len() != program_scenario.storage_capacity {
                return Err(SetupError::Storage(format!(
                    "`{name}` was generated {} slot(s), but it has {}",
                    storage.len(),
                    program_scenario.storage_capacity
                )));
            }

            let case = FuzzCase::new(self, &programs, &transaction, mutations, signer);
            let arguments = program_scenario.run_bindings(&program_scenario.argument_bindings, arguments, &case)?;

            let mut program = (program_scenario.build_program)(&arguments)
                .map_err(|error| SetupError::Arguments(format!("`{name}`: {error}")))?;
            program.set_storage(storage);
            let script = program.get_script_pubkey(signer.get_network());

            if !program_scenario.inputs.is_empty() {
                (program_scenario.check_witness)(&witness)
                    .map_err(|error| SetupError::Witness(format!("`{name}`: {error}")))?;
            }

            for &index in &program_scenario.inputs {
                let input = &mut transaction.inputs_mut()[index];

                input.partial_input.witness_utxo.script_pubkey = script.clone();
                input.program_input = Some(ProgramInput::new(Box::new(program.clone()), witness.clone()));
            }

            for &index in &program_scenario.outputs {
                transaction.outputs_mut()[index].script_pubkey = script.clone();
            }

            programs.push(program);
        }

        // Bind witnesses after each program is built.
        for program_scenario in &self.program_scenarios {
            let Some(witness) = program_scenario.witness_in(&transaction) else {
                continue;
            };

            let case = FuzzCase::new(self, &programs, &transaction, mutations, signer);
            let witness = program_scenario.run_bindings(&program_scenario.witness_bindings, witness.clone(), &case)?;

            program_scenario.place_witness(&mut transaction, witness);
        }

        // Set the required witness values, then violate one of them in some cases.
        let mut requirements = Requirements::new();

        for program_scenario in &self.program_scenarios {
            let Some(generated) = program_scenario.witness_in(&transaction).cloned() else {
                continue;
            };

            let case = FuzzCase::new(self, &programs, &transaction, mutations, signer);
            let required =
                program_scenario.run_bindings(&program_scenario.witness_requirements, generated.clone(), &case)?;

            requirements.record_diff(program_scenario, &generated, &required);
            program_scenario.place_witness(&mut transaction, required);
        }

        let violated = requirements.violate_one(change.violation(), &mut transaction);

        // Check that custom strategies, bindings and requirements don't leave the spending path.
        for program_scenario in &self.program_scenarios {
            let Some(witness) = program_scenario.witness_in(&transaction) else {
                continue;
            };

            for (field, path) in &program_scenario.spending_paths {
                if !witness.get(field).is_some_and(|value| follows_path(value, path)) {
                    return Err(SetupError::Witness(format!(
                        "`{}` leaves its spending path {path:?} in `{field}`",
                        program_scenario.name
                    )));
                }
            }
        }

        // Lock recursive outputs to each program with its next storage.
        for (program_scenario, program) in self.program_scenarios.iter().zip(&programs) {
            let name = &program_scenario.name;

            for (index, transition) in &program_scenario.transitions {
                let case = FuzzCase::new(self, &programs, &transaction, mutations, signer);

                let storage = panic::catch_unwind(AssertUnwindSafe(|| {
                    let storage = transition(&case);

                    assert!(
                        storage.len() == program_scenario.storage_capacity,
                        "The transition of `{name}` returned {} storage slot(s), but it has {}",
                        storage.len(),
                        program_scenario.storage_capacity
                    );

                    storage
                }))
                .map_err(|_| SetupError::Binding(name.clone()))?;

                let mut next = program.clone();
                next.set_storage(storage);

                transaction.outputs_mut()[*index].script_pubkey = next.get_script_pubkey(signer.get_network());
            }
        }

        Ok((programs, transaction, violated))
    }

    /// # Panics
    /// Panics when no program is named `name`, or it is not a `P`.
    pub(crate) fn program_scenario<P: 'static>(&self, name: &str) -> (usize, &FuzzProgramScenario) {
        let (index, program_scenario) = self
            .program_scenarios
            .iter()
            .enumerate()
            .find(|(_, program_scenario)| program_scenario.name == name)
            .unwrap_or_else(|| panic!("No program is named `{name}`"));

        let (type_id, program_name) = program_scenario.program_type;
        assert!(
            type_id == TypeId::of::<P>(),
            "`{name}` is a `{program_name}`, not a `{}`",
            type_name::<P>()
        );

        (index, program_scenario)
    }

    /// `None` for a fixed input.
    pub(crate) fn program_scenario_by_input(&self, input: usize) -> Option<&FuzzProgramScenario> {
        self.program_scenarios
            .iter()
            .find(|program_scenario| program_scenario.inputs.contains(&input))
    }

    /// Every output the lock check protects, with the name of the program it is locked to.
    pub(crate) fn locked_outputs(&self) -> Vec<(usize, &str)> {
        self.program_scenarios
            .iter()
            .flat_map(|program_scenario| {
                program_scenario
                    .outputs
                    .iter()
                    .filter(|index| !program_scenario.unchecked_outputs.contains(index))
                    .map(|&index| (index, program_scenario.name.as_str()))
            })
            .collect()
    }

    /// Each program and the fixed program inputs with their bound values.
    pub(crate) fn describe(&self, programs: &[Program], transaction: &FinalTransaction) -> String {
        self.program_scenarios
            .iter()
            .zip(programs)
            .map(|(program_scenario, program)| {
                let mut description = format!("`{}` arguments: {}", program_scenario.name, program.get_arguments());

                if let Some(witness) = program_scenario.witness_in(transaction) {
                    description += &format!(" and witness: {witness}");
                }

                if !program.get_storage().is_empty() {
                    let storage = program
                        .get_storage()
                        .iter()
                        .map(|slot| slot.iter().map(|byte| format!("{byte:02x}")).collect::<String>())
                        .collect::<Vec<_>>()
                        .join(", ");

                    description += &format!(" and storage: [{storage}]");
                }

                description
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Where each program sits, e.g. "vault spends input 0, unvault locks output 0".
    pub(crate) fn layout(&self) -> String {
        let programs = self.program_scenarios.iter().map(|program_scenario| {
            let roles = program_scenario
                .inputs
                .iter()
                .map(|index| format!("spends input {index}"))
                .chain(program_scenario.outputs.iter().map(|index| {
                    if program_scenario
                        .transitions
                        .iter()
                        .any(|(recursive, _)| recursive == index)
                    {
                        format!("locks output {index} recursively")
                    } else if program_scenario.unchecked_outputs.contains(index) {
                        format!("locks output {index} unchecked")
                    } else {
                        format!("locks output {index}")
                    }
                }))
                .collect::<Vec<_>>()
                .join(" and ");

            format!("{} {roles}", program_scenario.name)
        });

        let fixed = self
            .transaction
            .inputs()
            .iter()
            .enumerate()
            .filter(|(index, input)| input.program_input.is_some() && self.program_scenario_by_input(*index).is_none())
            .map(|(index, _)| format!("input {index} is fixed"));

        programs.chain(fixed).collect::<Vec<_>>().join(", ")
    }
}

impl Default for Scenario {
    /// A transaction with one default input.
    fn default() -> Self {
        let mut transaction = FinalTransaction::new();
        transaction.add_input(PartialInput::default(), RequiredSignature::None);

        Self::new(transaction)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use proptest::strategy::{Just, Strategy};

    use simplicityhl::elements::{OutPoint, Txid};
    use simplicityhl::types::TypeConstructible;
    use simplicityhl::value::ValueConstructible;
    use simplicityhl::{Arguments, ResolvedType, TemplateProgramWitness, Value, WitnessValues};

    use smplx_sdk::transaction::{PartialOutput, UTXO};

    use crate::fuzz::program_strategy::FuzzProgramStrategy;

    use super::*;

    fn program_scenario(name: &str, inputs: &[usize], outputs: &[usize]) -> FuzzProgramScenario {
        FuzzProgramScenario {
            name: name.to_string(),
            inputs: inputs.to_vec(),
            outputs: outputs.to_vec(),
            unchecked_outputs: Vec::new(),
            spending_paths: Vec::new(),
            witness_types: Vec::new(),
            transitions: Vec::new(),
            storage_capacity: 0,
            program_type: (TypeId::of::<()>(), "()"),
            strategy: FuzzProgramStrategy::Other(
                Just(FuzzProgramValues::from((
                    Arguments::default(),
                    WitnessValues::default(),
                )))
                .boxed(),
            ),
            argument_bindings: Vec::new(),
            witness_bindings: Vec::new(),
            witness_requirements: Vec::new(),
            build_program: |_| unreachable!(),
            check_witness: |_| unreachable!(),
        }
    }

    fn two_inputs_one_output() -> FinalTransaction {
        let mut transaction = FinalTransaction::new();

        for vout in 0..2 {
            let utxo = UTXO {
                outpoint: OutPoint::new(Txid::from_byte_array(Default::default()), vout),
                ..UTXO::default()
            };

            transaction.add_input(PartialInput::new(utxo), RequiredSignature::None);
        }

        transaction.add_output(PartialOutput::default());

        transaction
    }

    #[test]
    fn invalid_scenarios_are_rejected() {
        let cases = [
            (
                vec![program_scenario("a", &[2], &[])],
                "`a` spends input 2, but the transaction has 2 input(s)",
            ),
            (
                vec![program_scenario("a", &[0], &[1])],
                "`a` locks output 1, but the transaction has 1 output(s)",
            ),
            (
                vec![program_scenario("a", &[0], &[]), program_scenario("b", &[0], &[])],
                "input 0 is spent by both `a` and `b`",
            ),
            (
                vec![program_scenario("a", &[0], &[0]), program_scenario("b", &[], &[0])],
                "output 0 is locked by both `a` and `b`",
            ),
            (vec![program_scenario("a", &[], &[0])], "no program spends an input"),
            (
                vec![program_scenario("a", &[0], &[]), program_scenario("a", &[1], &[])],
                "two programs are named `a`",
            ),
            (
                {
                    let mut unknown_path = program_scenario("a", &[0], &[]);
                    unknown_path.spending_paths.push((
                        TemplateProgramWitness::witness_from_str("PATH"),
                        vec!["Left".to_string()],
                    ));

                    vec![unknown_path]
                },
                "`a` has no witness field `PATH` with the spending path",
            ),
            (
                {
                    let mut index_last = program_scenario("a", &[0], &[]);
                    index_last.spending_paths.push((
                        TemplateProgramWitness::witness_from_str("PATH"),
                        vec!["Left".to_string(), "0".to_string()],
                    ));

                    vec![index_last]
                },
                "which must end at an `Either` branch",
            ),
        ];

        for (program_scenarios, reason) in cases {
            let mut scenario = Scenario::new(two_inputs_one_output());
            scenario.program_scenarios = program_scenarios;

            let error = scenario.validate().unwrap_err().to_string();

            assert!(error.contains(reason), "`{error}` does not mention `{reason}`");
        }
    }

    fn nested_witness(second: u32) -> WitnessValues {
        let value = Value::right(ResolvedType::u32(), Value::tuple([Value::u32(1), Value::u32(second)]));

        WitnessValues::from_map(HashMap::from([(
            TemplateProgramWitness::witness_from_str("PATH"),
            value,
        )]))
    }

    #[test]
    fn violates_a_nested_value() {
        let program_scenario = program_scenario("program", &[0], &[]);
        let program = Program::new("fn main() {}", Arguments::default());

        let mut transaction = FinalTransaction::new();
        transaction.add_program_input(
            PartialInput::default(),
            ProgramInput::new(Box::new(program), nested_witness(5)),
            RequiredSignature::None,
        );

        let mut requirements = Requirements::new();
        requirements.record_diff(&program_scenario, &nested_witness(2), &nested_witness(5));

        let violated = requirements
            .violate_one(Some(0), &mut transaction)
            .expect("one value is required");

        assert_eq!(
            violated.to_string(),
            r#"`program` witness `PATH` at ["Right", "1"] is 2 instead of 5"#
        );
        assert_eq!(program_scenario.witness_in(&transaction), Some(&nested_witness(2)));
    }
}
