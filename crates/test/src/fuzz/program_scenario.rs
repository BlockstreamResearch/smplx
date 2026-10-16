use std::any::{TypeId, type_name};
use std::marker::PhantomData;

use proptest::prelude::Strategy;

use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::Program;

use crate::fuzz::program::{FuzzProgram, FuzzProgramSchema, FuzzProgramTrait};
use crate::fuzz::program_strategy::{FuzzProgramStrategy, FuzzProgramValues};
use crate::fuzz::spec::FuzzCase;

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
    /// Locked outputs the lock check leaves alone. Also listed in `outputs`.
    pub(crate) unchecked_outputs: Vec<usize>,
    pub(crate) transitions: Vec<(usize, Transition)>,
    pub(crate) storage_capacity: usize,
    pub(crate) program_type: (TypeId, &'static str),
    pub(crate) strategy: FuzzProgramStrategy,
    pub(crate) argument_bindings: Vec<Binding<Arguments>>,
    pub(crate) witness_bindings: Vec<Binding<WitnessValues>>,
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
            transitions: Vec::new(),
            storage_capacity: 0,
            program_type: (TypeId::of::<P>(), type_name::<P>()),
            strategy: FuzzProgramStrategy::Guided(FuzzProgramSchema::of::<P>()),
            argument_bindings: Vec::new(),
            witness_bindings: Vec::new(),
            build_program: P::build_program,
            check_witness: P::check_witness,
        }
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
    pub fn storage_capacity(self, capacity: usize) -> Self {
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
        self.program_scenario
            .witness_bindings
            .push(Box::new(move |witness, case| {
                let mut witness =
                    P::Witness::try_from(witness).unwrap_or_else(|error| panic!("Generated witness: {error}"));
                bind(&mut witness, case);

                witness.into()
            }));

        self
    }
}
