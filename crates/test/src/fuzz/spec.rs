use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;

use crate::fuzz::FuzzableProgram;
use crate::fuzz::outcome::SetupError;

/// What the spec says should happen to the spend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    Accept,
    Reject,
    /// The spec does not decide this case.
    Any,
}

/// Read-only view of one fuzz case, given to the spec before the program executes.
pub struct FuzzCase<'a, P: FuzzableProgram> {
    program: ProgramView<'a, P>,
    transaction: &'a PartiallySignedTransaction,
    program_inputs: &'a [usize],
    signer: &'a Signer,
}

/// One program's part of a case.
struct ProgramView<'a, P: FuzzableProgram> {
    arguments: P::Arguments,
    witness: P::Witness,
    raw_arguments: &'a Arguments,
    raw_witness: &'a WitnessValues,
}

impl<'a, P: FuzzableProgram> FuzzCase<'a, P> {
    pub(crate) fn new(
        raw_arguments: &'a Arguments,
        raw_witness: &'a WitnessValues,
        transaction: &'a PartiallySignedTransaction,
        program_inputs: &'a [usize],
        signer: &'a Signer,
    ) -> Result<Self, SetupError> {
        let program = ProgramView {
            arguments: P::Arguments::try_from(raw_arguments).map_err(SetupError::Arguments)?,
            witness: P::Witness::try_from(raw_witness).map_err(SetupError::Witness)?,
            raw_arguments,
            raw_witness,
        };

        Ok(Self {
            program,
            transaction,
            program_inputs,
            signer,
        })
    }

    pub fn arguments(&self) -> &P::Arguments {
        &self.program.arguments
    }

    /// The witness as generated, before any signature is injected.
    pub fn witness(&self) -> &P::Witness {
        &self.program.witness
    }

    pub fn raw_arguments(&self) -> &Arguments {
        self.program.raw_arguments
    }

    pub fn raw_witness(&self) -> &WitnessValues {
        self.program.raw_witness
    }

    /// The transaction as signed. Program inputs carry no final witness yet.
    pub fn transaction(&self) -> &PartiallySignedTransaction {
        self.transaction
    }

    /// Indices of the inputs that execute the program.
    pub fn program_inputs(&self) -> &[usize] {
        self.program_inputs
    }

    pub fn network(&self) -> &SimplicityNetwork {
        self.signer.get_network()
    }

    /// The signer that signed the transaction.
    pub fn signer(&self) -> &Signer {
        self.signer
    }
}
