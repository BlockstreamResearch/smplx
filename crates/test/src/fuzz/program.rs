use simplicityhl::elements::Script;
use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::{Program, ProgramFactory, ProgramSchema};
use smplx_sdk::provider::SimplicityNetwork;

use crate::fuzz::mutations::UtxoMap;
use crate::fuzz::program_scenario::FuzzProgramScenario;

/// A program the fuzz engine can build, describe and decode. Implemented for every `include_simf!()` program.
pub trait FuzzProgramTrait:
    AsRef<Program>
    + ProgramFactory<Self>
    + ProgramSchema<
        Arguments: for<'a> TryFrom<&'a Arguments, Error = String> + Into<Arguments>,
        Witness: for<'a> TryFrom<&'a WitnessValues, Error = String> + Into<WitnessValues>,
    > + Clone
    + 'static
{
    /// Builds and compiles the program.
    ///
    /// # Errors
    /// Returns why the arguments do not fit the program.
    fn build_program(arguments: &Arguments) -> Result<Program, String> {
        Self::Arguments::try_from(arguments)?;

        let program = (*Self::instantiate_program(arguments.clone())).as_ref().clone();
        program.load().map_err(|error| error.to_string())?;

        Ok(program)
    }

    /// # Errors
    /// Returns why the witness does not fit the program.
    fn check_witness(witness: &WitnessValues) -> Result<(), String> {
        Self::Witness::try_from(witness).map(|_| ())
    }
}

impl<P> FuzzProgramTrait for P where
    P: AsRef<Program>
        + ProgramFactory<P>
        + ProgramSchema<
            Arguments: for<'a> TryFrom<&'a Arguments, Error = String> + Into<Arguments>,
            Witness: for<'a> TryFrom<&'a WitnessValues, Error = String> + Into<WitnessValues>,
        > + Clone
        + 'static
{
}

/// One fuzzed program as built for a case.
pub struct FuzzProgram<'a, P: FuzzProgramTrait> {
    program_scenario: &'a FuzzProgramScenario,
    program: &'a Program,
    network: &'a SimplicityNetwork,
    arguments: P::Arguments,
    witness: Option<P::Witness>,
    raw_witness: Option<&'a WitnessValues>,
    inputs: Vec<usize>,
    outputs: Vec<usize>,
}

impl<'a, P: FuzzProgramTrait> FuzzProgram<'a, P> {
    /// `raw_witness` is `None` when the program spends no input.
    pub(crate) fn new(
        program_scenario: &'a FuzzProgramScenario,
        program: &'a Program,
        raw_witness: Option<&'a WitnessValues>,
        map: &UtxoMap,
        network: &'a SimplicityNetwork,
    ) -> Self {
        Self {
            program_scenario,
            program,
            network,
            arguments: P::Arguments::try_from(program.get_arguments()).expect("checked when the program was built"),
            witness: raw_witness
                .map(|witness| P::Witness::try_from(witness).expect("checked when the program was built")),
            raw_witness,
            inputs: program_scenario.inputs_in(map),
            outputs: program_scenario.outputs_in(map),
        }
    }

    /// The name the program was added with.
    pub fn name(&self) -> &str {
        self.program_scenario.name()
    }

    /// The program's arguments, as generated and bound.
    pub fn arguments(&self) -> &P::Arguments {
        &self.arguments
    }

    /// The witness as generated, before any signature is injected.
    ///
    /// # Panics
    /// Panics when the program spends no input.
    pub fn witness(&self) -> &P::Witness {
        self.witness
            .as_ref()
            .unwrap_or_else(|| panic!("`{}` spends no input, so it has no witness", self.name()))
    }

    /// The program's arguments, as [`Arguments`].
    pub fn raw_arguments(&self) -> &Arguments {
        self.program.get_arguments()
    }

    /// The witness, as [`WitnessValues`].
    ///
    /// # Panics
    /// Panics when the program spends no input.
    pub fn raw_witness(&self) -> &WitnessValues {
        self.raw_witness
            .unwrap_or_else(|| panic!("`{}` spends no input, so it has no witness", self.name()))
    }

    /// The compiled program.
    pub fn program(&self) -> &Program {
        self.program
    }

    /// The storage the program is spent with.
    pub fn storage(&self) -> Vec<[u8; 32]> {
        self.program
            .get_storage()
            .iter()
            .map(|slot| slot.as_slice().try_into().expect("a storage slot is 32 bytes"))
            .collect()
    }

    /// The script of the program's address on the case's network.
    pub fn script_pubkey(&self) -> Script {
        self.program.get_script_pubkey(self.network)
    }

    /// The SHA-256 hash of [`FuzzProgram::script_pubkey`].
    pub fn script_hash(&self) -> [u8; 32] {
        self.program.get_script_hash(self.network)
    }

    /// The program's CMR.
    pub fn cmr(&self) -> [u8; 32] {
        self.program.get_cmr()
    }

    /// Indices of the inputs the program spends, after the mutations.
    pub fn inputs(&self) -> &[usize] {
        &self.inputs
    }

    /// Indices of the outputs the program locks, after the mutations.
    pub fn outputs(&self) -> &[usize] {
        &self.outputs
    }
}
