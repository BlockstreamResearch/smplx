use smplx_sdk::program::Program;
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::FinalTransaction;

use crate::fuzz::fuzz_program::{FuzzProgram, FuzzProgramTrait};
use crate::fuzz::scenario::Scenario;

/// What is expected to happen to the spend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Expect {
    Accept,
    Reject,
    Any,
}

/// Read-only view of one fuzz case, given to the spec before the programs execute.
pub struct FuzzCase<'a> {
    scenario: &'a Scenario,
    programs: &'a [Program],
    transaction: &'a FinalTransaction,
    signer: &'a Signer,
}

impl<'a> FuzzCase<'a> {
    pub(crate) fn new(
        scenario: &'a Scenario,
        programs: &'a [Program],
        transaction: &'a FinalTransaction,
        signer: &'a Signer,
    ) -> Self {
        Self {
            scenario,
            programs,
            transaction,
            signer,
        }
    }

    /// # Panics
    /// Panics when no program is named `name`, or it is not a `P`.
    pub fn program<P: FuzzProgramTrait>(&self, name: &str) -> FuzzProgram<'_, P> {
        let (index, program_scenario) = self.scenario.program_scenario::<P>(name);
        let program = &self.programs[index];

        let raw_witness = program_scenario.inputs.first().map(|&input| {
            &self.transaction.inputs()[input]
                .program_input
                .as_ref()
                .expect("a spent input runs its program")
                .witness
        });

        FuzzProgram::new(program_scenario, program, raw_witness, self.signer.get_network())
    }

    /// The initial transaction with every program before signing.
    pub fn transaction(&self) -> &FinalTransaction {
        self.transaction
    }

    pub fn network(&self) -> &SimplicityNetwork {
        self.signer.get_network()
    }

    /// The signer that signed the transaction.
    pub fn signer(&self) -> &Signer {
        self.signer
    }
}
