use std::collections::HashMap;

use simplicityhl::WitnessValues;
use simplicityhl::elements::pset::PartiallySignedTransaction;

use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::FinalTransaction;

use crate::fuzz::outcome::{Outcome, SetupError};

/// A transaction signed by one signer, ready to run its program inputs.
pub(crate) struct SignedTransaction<'a> {
    transaction: &'a FinalTransaction,
    network: &'a SimplicityNetwork,
    pst: PartiallySignedTransaction,
    witnesses: HashMap<usize, WitnessValues>,
}

impl<'a> SignedTransaction<'a> {
    /// # Errors
    /// Returns [`SetupError::Signing`] when `signer` cannot sign `transaction`.
    pub(crate) fn new(signer: &'a Signer, transaction: &'a FinalTransaction) -> Result<Self, SetupError> {
        let (pst, witnesses) = signer
            .sign_witnesses(transaction)
            .map_err(|error| SetupError::Signing(error.to_string()))?;

        Ok(Self {
            transaction,
            network: signer.get_network(),
            pst,
            witnesses,
        })
    }

    /// Runs the program of input `index`.
    ///
    /// # Errors
    /// Returns a [`SetupError`] when the failure comes from the fuzz setup rather than the program.
    ///
    /// # Panics
    /// Panics when input `index` runs no program.
    pub(crate) fn run(&self, index: usize) -> Result<Outcome, SetupError> {
        let program = &self.transaction.inputs()[index]
            .program_input
            .as_ref()
            .expect("the input runs a program")
            .program;

        Outcome::classify(program.finalize(&self.pst, &self.witnesses[&index], index, self.network))
    }

    /// Runs every program input, in order.
    ///
    /// # Errors
    /// Returns a [`SetupError`] when a failure comes from the fuzz setup rather than a program.
    pub(crate) fn run_all(&self) -> Result<Vec<(usize, Outcome)>, SetupError> {
        self.program_inputs()
            .map(|index| Ok((index, self.run(index)?)))
            .collect()
    }

    /// Whether every program input accepts, stopping at the first that rejects.
    ///
    /// # Errors
    /// Returns a [`SetupError`] when a failure comes from the fuzz setup rather than a program.
    pub(crate) fn accepts_all(&self) -> Result<bool, SetupError> {
        for index in self.program_inputs() {
            if !self.run(index)?.is_accepted() {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn program_inputs(&self) -> impl Iterator<Item = usize> + '_ {
        self.transaction
            .inputs()
            .iter()
            .enumerate()
            .filter_map(|(index, input)| input.program_input.as_ref().map(|_| index))
    }
}
