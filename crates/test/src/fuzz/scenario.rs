use std::any::{TypeId, type_name};
use std::collections::{HashMap, HashSet};
use std::panic::{self, AssertUnwindSafe};

use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::Program;
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};

use crate::fuzz::FuzzProgramTrait;
use crate::fuzz::outcome::SetupError;
use crate::fuzz::program_scenario::{FuzzProgramScenario, FuzzProgramScenarioBuilder};
use crate::fuzz::scenario_strategy::Combined;
use crate::fuzz::spec::FuzzCase;

type Values = (Arguments, WitnessValues);

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

            if program_scenario.inputs.is_empty() && !program_scenario.witness_bindings.is_empty() {
                return invalid(format!("`{name}` binds its witness, but spends no input"));
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

    /// Binds and builds every program in order and places it into the transaction, then binds the witnesses.
    ///
    /// # Errors
    /// Returns a [`SetupError`] when the values do not fit a program or a binding panics.
    pub(crate) fn build(
        &self,
        values: Vec<Values>,
        signer: &Signer,
    ) -> Result<(Vec<Program>, FinalTransaction), SetupError> {
        let mut programs = Vec::with_capacity(self.program_scenarios.len());
        let mut transaction = self.transaction.clone();

        for (program_scenario, (mut arguments, witness)) in self.program_scenarios.iter().zip(values) {
            let name = &program_scenario.name;

            // Bind arguments.
            for binding in &program_scenario.argument_bindings {
                let case = FuzzCase::new(self, &programs, &transaction, signer);

                arguments = panic::catch_unwind(AssertUnwindSafe(|| binding(&arguments, &case)))
                    .map_err(|_| SetupError::Binding(name.clone()))?;
            }

            let program = (program_scenario.build_program)(&arguments)
                .map_err(|error| SetupError::Arguments(format!("`{name}`: {error}")))?;
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
            let name = &program_scenario.name;

            for binding in &program_scenario.witness_bindings {
                let case = FuzzCase::new(self, &programs, &transaction, signer);
                let witness = &transaction.inputs()[program_scenario.inputs[0]]
                    .program_input
                    .as_ref()
                    .expect("a spent input runs its program")
                    .witness;

                let witness = panic::catch_unwind(AssertUnwindSafe(|| binding(witness, &case)))
                    .map_err(|_| SetupError::Binding(name.clone()))?;

                for &index in &program_scenario.inputs {
                    let input = transaction.inputs_mut()[index]
                        .program_input
                        .as_mut()
                        .expect("a spent input runs its program");

                    input.witness = witness.clone();
                }
            }
        }

        Ok((programs, transaction))
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

    pub(crate) fn combined_strategy(&self) -> Combined {
        Combined::new(
             self.program_scenarios
                .iter()
                .map(|program_scenario| program_scenario.strategy.clone())
                .collect(),
        )
    }

    /// Each program and the fixed program inputs with their bound values.
    pub(crate) fn describe(&self, programs: &[Program], transaction: &FinalTransaction) -> String {
        self.program_scenarios
            .iter()
            .zip(programs)
            .map(|(program_scenario, program)| match program_scenario.inputs.first() {
                Some(&input) => {
                    let witness = &transaction.inputs()[input]
                        .program_input
                        .as_ref()
                        .expect("an input runs its program")
                        .witness;

                    format!(
                        "`{}` arguments: {} and witness: {witness}",
                        program_scenario.name,
                        program.get_arguments()
                    )
                }
                None => format!("`{}` arguments: {}", program_scenario.name, program.get_arguments()),
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
                .chain(
                    program_scenario
                        .outputs
                        .iter()
                        .map(|index| format!("locks output {index}")),
                )
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
        transaction.add_input(PartialInput::new(UTXO::default()), RequiredSignature::None);

        Self::new(transaction)
    }
}

#[cfg(test)]
mod tests {
    use proptest::strategy::{Just, Strategy};

    use simplicityhl::elements::{OutPoint, Script, Txid};

    use smplx_sdk::provider::SimplicityNetwork;
    use smplx_sdk::transaction::PartialOutput;

    use crate::fuzz::program_strategy::ProgramStrategy;

    use super::*;

    fn program_scenario(name: &str, inputs: &[usize], outputs: &[usize]) -> FuzzProgramScenario {
        FuzzProgramScenario {
            name: name.to_string(),
            inputs: inputs.to_vec(),
            outputs: outputs.to_vec(),
            program_type: (TypeId::of::<()>(), "()"),
            strategy: ProgramStrategy::Other(Just((Arguments::default(), WitnessValues::default())).boxed()),
            argument_bindings: Vec::new(),
            witness_bindings: Vec::new(),
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

        transaction.add_output(PartialOutput::new(
            Script::new(),
            1,
            SimplicityNetwork::default_regtest().policy_asset(),
        ));

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
        ];

        for (program_scenarios, reason) in cases {
            let mut scenario = Scenario::new(two_inputs_one_output());
            scenario.program_scenarios = program_scenarios;

            let error = scenario.validate().unwrap_err().to_string();

            assert!(error.contains(reason), "`{error}` does not mention `{reason}`");
        }
    }
}
