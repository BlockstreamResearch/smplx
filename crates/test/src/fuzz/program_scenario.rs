use std::any::{TypeId, type_name};
use std::marker::PhantomData;

use proptest::prelude::Strategy;

use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::Program;

use crate::fuzz::FuzzProgramTrait;
use crate::fuzz::program_strategy::{GuidedSchema, ProgramStrategy, Random};
use crate::fuzz::spec::FuzzCase;

type Values = (Arguments, WitnessValues);
type Binding<T> = Box<dyn Fn(&T, &FuzzCase<'_>) -> T>;

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
    pub(crate) program_type: (TypeId, &'static str),
    pub(crate) strategy: ProgramStrategy,
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
            program_type: (TypeId::of::<P>(), type_name::<P>()),
            strategy: ProgramStrategy::Guided(GuidedSchema::of::<P>()),
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

    /// Generates the program's arguments and witness uniformly at random.
    pub fn random_strategy(self) -> Self {
        self.program_scenario.strategy = ProgramStrategy::Random(Random::<P>::default().boxed());

        self
    }

    /// Replaces how the program's arguments and witness are generated.
    pub fn custom_strategy(self, strategy: impl Strategy<Value = Values> + 'static) -> Self {
        self.program_scenario.strategy = ProgramStrategy::Other(strategy.boxed());

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
