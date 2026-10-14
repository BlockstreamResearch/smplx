use std::any::{TypeId, type_name};
use std::collections::{HashMap, HashSet};

use proptest::prelude::{BoxedStrategy, Strategy};

use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::Program;
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};

use crate::fuzz::FuzzProgramTrait;
use crate::fuzz::args_strategy::ArgsStrategyBuilder;
use crate::fuzz::outcome::SetupError;

type Values = (Arguments, WitnessValues);

/// A transaction and the programs being fuzzed.
pub struct Scenario {
    transaction: FinalTransaction,
    program_scenarios: Vec<FuzzProgramScenario>,
}

/// A builder to configure what each FuzzProgram does.
pub struct FuzzProgramScenarioBuilder<'a> {
    program_scenario: &'a mut FuzzProgramScenario,
}

/// The FuzzProgram fuzz configuration.
pub(crate) struct FuzzProgramScenario {
    pub(crate) name: String,
    pub(crate) inputs: Vec<usize>,
    pub(crate) outputs: Vec<usize>,
    program_type: (TypeId, &'static str),
    strategy: BoxedStrategy<Values>,
    build_program: fn(&Arguments) -> Result<Program, String>,
    check_witness: fn(&WitnessValues) -> Result<(), String>,
}

impl Scenario {
    /// Fuzzes the programs added to `transaction`. Everything they do not override stays as assembled.
    pub fn new(transaction: FinalTransaction) -> Self {
        Self {
            transaction,
            program_scenarios: Vec::new(),
        }
    }

    pub fn add<P: FuzzProgramTrait>(&mut self, name: impl Into<String>) -> FuzzProgramScenarioBuilder<'_> {
        self.program_scenarios.push(FuzzProgramScenario {
            name: name.into(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            program_type: (TypeId::of::<P>(), type_name::<P>()),
            strategy: ArgsStrategyBuilder::<P>::new().build(),
            build_program: P::build_program,
            check_witness: P::check_witness,
        });

        FuzzProgramScenarioBuilder {
            program_scenario: self.program_scenarios.last_mut().expect("just added"),
        }
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

    /// Builds every program and places it into the transaction.
    ///
    /// # Errors
    /// Returns a [`SetupError`] when the values do not fit a program.
    pub(crate) fn build(
        &self,
        values: Vec<Values>,
        network: &SimplicityNetwork,
    ) -> Result<(Vec<Program>, FinalTransaction), SetupError> {
        let mut programs = Vec::with_capacity(self.program_scenarios.len());
        let mut transaction = self.transaction.clone();

        for (program_scenario, (arguments, witness)) in self.program_scenarios.iter().zip(values) {
            let name = &program_scenario.name;

            let program = (program_scenario.build_program)(&arguments)
                .map_err(|error| SetupError::Arguments(format!("`{name}`: {error}")))?;
            let script = program.get_script_pubkey(network);

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

    pub(crate) fn program_scenario_names(&self) -> impl Iterator<Item = &str> {
        self.program_scenarios
            .iter()
            .map(|program_scenario| program_scenario.name.as_str())
    }

    pub(crate) fn program_scenario_strategies(&self) -> Vec<BoxedStrategy<Values>> {
        self.program_scenarios
            .iter()
            .map(|program_scenario| program_scenario.strategy.clone())
            .collect()
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

impl FuzzProgramScenarioBuilder<'_> {
    /// Spends the transaction input at `index`, overriding its arguments and witness.
    pub fn spends(self, index: usize) -> Self {
        self.program_scenario.inputs.push(index);

        self
    }

    /// Locks the transaction output at `index`, overriding its script.
    pub fn locks(self, index: usize) -> Self {
        self.program_scenario.outputs.push(index);

        self
    }

    /// Replaces how the program's arguments and witness are generated.
    pub fn strategy(self, strategy: impl Strategy<Value = Values> + 'static) -> Self {
        self.program_scenario.strategy = strategy.boxed();

        self
    }
}

#[cfg(test)]
mod tests {
    use proptest::strategy::Just;

    use simplicityhl::elements::{OutPoint, Script, Txid};

    use smplx_sdk::transaction::PartialOutput;

    use super::*;

    fn program_scenario(name: &str, inputs: &[usize], outputs: &[usize]) -> FuzzProgramScenario {
        FuzzProgramScenario {
            name: name.to_string(),
            inputs: inputs.to_vec(),
            outputs: outputs.to_vec(),
            program_type: (TypeId::of::<()>(), "()"),
            strategy: Just((Arguments::default(), WitnessValues::default())).boxed(),
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
