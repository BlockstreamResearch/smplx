use std::any::{TypeId, type_name};
use std::marker::PhantomData;
use std::panic::{self, AssertUnwindSafe};

use proptest::prelude::Strategy;

use simplicityhl::num::U256;
use simplicityhl::{Arguments, ResolvedType, TemplateProgramWitness, WitnessValues};

use smplx_sdk::program::Program;
use smplx_sdk::transaction::FinalTransaction;

use crate::fuzz::case::FuzzCase;
use crate::fuzz::mutations::UtxoMap;
use crate::fuzz::outcome::SetupError;
use crate::fuzz::program::{FuzzProgram, FuzzProgramTrait};
use crate::fuzz::program_strategy::{FuzzProgramSchema, FuzzProgramStrategy, FuzzProgramValues};
use crate::fuzz::witness_path::WitnessPath;

type Binding<T> = Box<dyn Fn(&T, &FuzzCase<'_>) -> T>;
type Transition = Box<dyn Fn(&FuzzCase<'_>) -> Vec<[u8; 32]>>;

/// A builder to configure what each FuzzProgram does.
pub struct FuzzProgramScenarioBuilder<'a, P> {
    program_scenario: &'a mut FuzzProgramScenario,
    _program: PhantomData<fn() -> P>,
}

/// The FuzzProgram fuzz configuration.
pub(crate) struct FuzzProgramScenario {
    pub(crate) name: String,
    pub(crate) inputs: Vec<usize>,
    pub(crate) outputs: Vec<usize>,
    pub(crate) unchecked_outputs: Vec<usize>,
    /// Witness fields with the branches the witness must take, e.g. `("PATH", ["Right", "Left"])`.
    pub(crate) spending_paths: Vec<(TemplateProgramWitness, WitnessPath)>,
    pub(crate) witness_types: Vec<(TemplateProgramWitness, ResolvedType)>,
    pub(crate) transitions: Vec<(usize, Transition)>,
    pub(crate) storage_capacity: usize,
    pub(crate) program_type: (TypeId, &'static str),
    pub(crate) strategy: FuzzProgramStrategy,
    pub(crate) argument_bindings: Vec<Binding<Arguments>>,
    pub(crate) witness_bindings: Vec<Binding<WitnessValues>>,
    pub(crate) witness_requirements: Vec<Binding<WitnessValues>>,
    pub(crate) build_program: fn(&Arguments) -> Result<Program, String>,
    pub(crate) check_witness: fn(&WitnessValues) -> Result<(), String>,
}

impl FuzzProgramScenario {
    pub(crate) fn new<P: FuzzProgramTrait>(name: String) -> Self {
        Self {
            name,
            inputs: Vec::new(),
            outputs: Vec::new(),
            unchecked_outputs: Vec::new(),
            spending_paths: Vec::new(),
            witness_types: P::witness_types(),
            transitions: Vec::new(),
            storage_capacity: 0,
            program_type: (TypeId::of::<P>(), type_name::<P>()),
            strategy: FuzzProgramStrategy::Guided(FuzzProgramSchema::of::<P>()),
            argument_bindings: Vec::new(),
            witness_bindings: Vec::new(),
            witness_requirements: Vec::new(),
            build_program: P::build_program,
            check_witness: P::check_witness,
        }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Whether the program spends input `index` of the assembled transaction.
    pub(crate) fn spends(&self, index: usize) -> bool {
        self.inputs.contains(&index)
    }

    /// Whether the program locks output `index` of the assembled transaction.
    pub(crate) fn locks(&self, index: usize) -> bool {
        self.outputs.contains(&index)
    }

    pub(crate) fn has_requirements(&self) -> bool {
        !self.witness_requirements.is_empty()
    }

    /// The literals of the program's source, empty for a custom strategy.
    pub(crate) fn literals(&self) -> &[U256] {
        self.strategy.schema().map_or(&[], |schema| &schema.literals)
    }

    /// The strategy generating this program's values, with its storage capacity and spending paths.
    pub(crate) fn configured_strategy(&self) -> FuzzProgramStrategy {
        match &self.strategy {
            FuzzProgramStrategy::Guided(schema) => FuzzProgramStrategy::Guided(self.configure(schema)),
            FuzzProgramStrategy::Random(schema) => FuzzProgramStrategy::Random(self.configure(schema)),
            FuzzProgramStrategy::Other(strategy) => FuzzProgramStrategy::Other(strategy.clone()),
        }
    }

    /// Runs `bindings` of this program on `value` in order.
    pub(crate) fn run_bindings<T>(
        &self,
        bindings: &[Binding<T>],
        value: T,
        case: &FuzzCase<'_>,
    ) -> Result<T, SetupError> {
        bindings.iter().try_fold(value, |value, binding| {
            panic::catch_unwind(AssertUnwindSafe(|| binding(&value, case)))
                .map_err(|_| SetupError::Binding(self.name.clone()))
        })
    }

    /// Where the inputs this program spends sit after the mutations.
    pub(crate) fn inputs_in(&self, map: &UtxoMap) -> Vec<usize> {
        self.inputs.iter().map(|&input| map.new_input_position(input)).collect()
    }

    /// Where the outputs this program locks sit after the mutations.
    pub(crate) fn outputs_in(&self, map: &UtxoMap) -> Vec<usize> {
        self.outputs
            .iter()
            .map(|&output| map.new_output_position(output))
            .collect()
    }

    /// The witness of the inputs this program spends, `None` when it spends nothing.
    pub(crate) fn witness_in<'t>(&self, transaction: &'t FinalTransaction, map: &UtxoMap) -> Option<&'t WitnessValues> {
        self.inputs.first().map(|&input| {
            &transaction.inputs()[map.new_input_position(input)]
                .program_input
                .as_ref()
                .expect("a spent input runs its program")
                .witness
        })
    }

    /// Places `witness` into every input this program spends.
    pub(crate) fn place_witness(&self, transaction: &mut FinalTransaction, map: &UtxoMap, witness: WitnessValues) {
        for input in self.inputs_in(map) {
            transaction.inputs_mut()[input]
                .program_input
                .as_mut()
                .expect("a spent input runs its program")
                .witness = witness.clone();
        }
    }

    fn configure(&self, schema: &FuzzProgramSchema) -> FuzzProgramSchema {
        schema
            .clone()
            .with_storage_capacity(self.storage_capacity)
            .with_spending_paths(self.spending_paths.clone())
    }
}

impl<'a, P: FuzzProgramTrait> FuzzProgramScenarioBuilder<'a, P> {
    pub(crate) fn new(program_scenario: &'a mut FuzzProgramScenario) -> Self {
        Self {
            program_scenario,
            _program: PhantomData,
        }
    }

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

    /// Locks the transaction output at `index` like [`Self::locks`] but doesn't do the "redirect" check.
    /// E.g. for a spend that may legitimately send the coins anywhere.
    pub fn locks_unchecked(self, index: usize) -> Self {
        self.program_scenario.outputs.push(index);
        self.program_scenario.unchecked_outputs.push(index);

        self
    }

    /// Specifies the spending path of the program to fuzz. The path must exist in witness
    /// and end with "Left" or "Right".
    pub fn spending_path<I>(self, name: &str, path: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.program_scenario
            .spending_paths
            .push((TemplateProgramWitness::witness_from_str(name), WitnessPath::new(path)));

        self
    }

    /// Locks the transaction output at `index` to this program with the storage `transition` function returns.
    pub fn recursive(self, index: usize, transition: impl Fn(&FuzzProgram<'_, P>) -> Vec<[u8; 32]> + 'static) -> Self {
        let name = self.program_scenario.name.clone();

        self.program_scenario.outputs.push(index);
        self.program_scenario
            .transitions
            .push((index, Box::new(move |case| transition(&case.program::<P>(&name)))));

        self
    }

    /// Gives the program `capacity` storage slots, generated like its other values.
    ///
    /// # Panics
    /// Panics if `capacity` exceeds [`Program::MAX_STORAGE_SLOTS`].
    pub fn storage_capacity(self, capacity: usize) -> Self {
        // Refused here rather than in the middle of a fuzz run
        assert!(
            capacity <= Program::MAX_STORAGE_SLOTS,
            "A program holds at most {} storage slots, and `{}` asks for {capacity}",
            Program::MAX_STORAGE_SLOTS,
            self.program_scenario.name
        );

        self.program_scenario.storage_capacity = capacity;

        self
    }

    /// Generates the program's arguments and witness uniformly at random.
    pub fn random_strategy(self) -> Self {
        self.program_scenario.strategy = FuzzProgramStrategy::Random(FuzzProgramSchema::of::<P>());

        self
    }

    /// Replaces how the program's arguments and witness are generated.
    pub fn custom_strategy(self, strategy: impl Strategy<Value = FuzzProgramValues> + 'static) -> Self {
        self.program_scenario.strategy = FuzzProgramStrategy::Other(strategy.boxed());

        self
    }

    /// Sets argument fields before the program is built. `case` holds the programs added before this one.
    pub fn bind_arguments(self, bind: impl Fn(&mut P::Arguments, &FuzzCase<'_>) + 'static) -> Self {
        self.program_scenario
            .argument_bindings
            .push(Box::new(move |arguments, case| {
                let mut arguments =
                    P::Arguments::try_from(arguments).unwrap_or_else(|error| panic!("Generated arguments: {error}"));
                bind(&mut arguments, case);

                arguments.into()
            }));

        self
    }

    /// Sets witness fields once every program is built and placed before signing.
    pub fn bind_witness(self, bind: impl Fn(&mut P::Witness, &FuzzCase<'_>) + 'static) -> Self {
        self.program_scenario.witness_bindings.push(Self::witness_binding(bind));

        self
    }

    /// Sets the only witness values the contract accepts, after the bindings.
    /// Required witness is fuzzed and the contract should reject the alternatives.
    pub fn requires_witness(self, require: impl Fn(&mut P::Witness, &FuzzCase<'_>) + 'static) -> Self {
        self.program_scenario
            .witness_requirements
            .push(Self::witness_binding(require));

        self
    }

    fn witness_binding(bind: impl Fn(&mut P::Witness, &FuzzCase<'_>) + 'static) -> Binding<WitnessValues> {
        Box::new(move |witness, case| {
            let mut witness =
                P::Witness::try_from(witness).unwrap_or_else(|error| panic!("Generated witness: {error}"));
            bind(&mut witness, case);

            witness.into()
        })
    }
}
