use smplx_sdk::program::Program;
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::FinalTransaction;
use smplx_sdk::utils::hash_script;

use crate::fuzz::program::{FuzzProgram, FuzzProgramTrait};
use crate::fuzz::scenario::Scenario;

/// What is expected to happen to the spend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Expect {
    Accept,
    Reject,
    Any,
}

/// Read-only view of one fuzz case, given to bindings while it is built and to the spec before it executes.
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
    /// Panics when no program is named `name`, it is not a `P`, or a binding reads it before it is built.
    pub fn program<P: FuzzProgramTrait>(&self, name: &str) -> FuzzProgram<'_, P> {
        let (index, program_scenario) = self.scenario.program_scenario::<P>(name);
        let program = self
            .programs
            .get(index)
            .unwrap_or_else(|| panic!("`{name}` is not built yet, only programs added before a binding can be read"));

        FuzzProgram::new(
            program_scenario,
            program,
            program_scenario.witness_in(self.transaction),
            self.signer.get_network(),
        )
    }

    /// The initial transaction with every program before signing.
    pub fn transaction(&self) -> &FinalTransaction {
        self.transaction
    }

    /// # Panics
    /// Panics when the transaction has no input `index`.
    pub fn input_script_hash(&self, index: usize) -> [u8; 32] {
        hash_script(
            &self.transaction.inputs()[index]
                .partial_input
                .witness_utxo
                .script_pubkey,
        )
    }

    /// # Panics
    /// Panics when the transaction has no input `index` or its asset is confidential.
    pub fn input_asset(&self, index: usize) -> [u8; 32] {
        self.transaction.inputs()[index]
            .partial_input
            .asset
            .expect("the input asset is confidential")
            .to_byte_array()
    }

    /// # Panics
    /// Panics when the transaction has no input `index` or its amount is confidential.
    pub fn input_amount(&self, index: usize) -> u64 {
        self.transaction.inputs()[index]
            .partial_input
            .amount
            .expect("the input amount is confidential")
    }

    /// # Panics
    /// Panics when the transaction has no output `index`.
    pub fn output_script_hash(&self, index: usize) -> [u8; 32] {
        hash_script(&self.transaction.outputs()[index].script_pubkey)
    }

    /// # Panics
    /// Panics when the transaction has no output `index`.
    pub fn output_asset(&self, index: usize) -> [u8; 32] {
        self.transaction.outputs()[index].asset.to_byte_array()
    }

    /// # Panics
    /// Panics when the transaction has no output `index`.
    pub fn output_amount(&self, index: usize) -> u64 {
        self.transaction.outputs()[index].amount
    }

    pub fn network(&self) -> &SimplicityNetwork {
        self.signer.get_network()
    }

    /// The signer that signs the transaction.
    pub fn signer(&self) -> &Signer {
        self.signer
    }
}
