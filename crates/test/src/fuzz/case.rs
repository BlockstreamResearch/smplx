use simplicityhl::elements::Sequence;
use simplicityhl::elements::locktime::LOCK_TIME_THRESHOLD;

use smplx_sdk::program::Program;
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::FinalTransaction;
use smplx_sdk::utils::hash_script;

use crate::fuzz::mutations::{Mutation, UtxoMap};
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
    mutations: &'a [Mutation],
    map: UtxoMap,
    signer: &'a Signer,
}

impl<'a> FuzzCase<'a> {
    pub(crate) fn new(
        scenario: &'a Scenario,
        programs: &'a [Program],
        transaction: &'a FinalTransaction,
        mutations: &'a [Mutation],
        signer: &'a Signer,
    ) -> Self {
        Self {
            scenario,
            programs,
            transaction,
            mutations,
            map: scenario.utxo_map(mutations),
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
            program_scenario.witness_in(self.transaction, &self.map),
            &self.map,
            self.signer.get_network(),
        )
    }

    /// The transaction with the mutations applied and every program placed, before signing.
    pub fn transaction(&self) -> &FinalTransaction {
        self.transaction
    }

    /// What the case changed in the assembled transaction.
    pub fn mutations(&self) -> &[Mutation] {
        self.mutations
    }

    /// Where input `old_index` of the assembled transaction sits after the mutations.
    ///
    /// # Panics
    /// Panics when the assembled transaction has no input `old_index`.
    pub fn input_index(&self, old_index: usize) -> usize {
        self.map.new_input_position(old_index)
    }

    /// Where output `old_index` of the assembled transaction sits after the mutations.
    ///
    /// # Panics
    /// Panics when the assembled transaction has no output `old_index`.
    pub fn output_index(&self, old_index: usize) -> usize {
        self.map.new_output_position(old_index)
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

    /// The sequence input `index` is signed with: its own, or the transaction's.
    ///
    /// # Panics
    /// Panics when the transaction has no input `index`.
    pub fn input_sequence(&self, index: usize) -> Sequence {
        self.transaction.inputs()[index]
            .partial_input
            .sequence
            .unwrap_or(self.transaction.sequence())
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

    /// The lock height as the jets see it: zero when every input sequence is final or the locktime is a time.
    pub fn lock_height(&self) -> u32 {
        self.read_lock(|locktime| locktime < LOCK_TIME_THRESHOLD)
    }

    /// The lock time as the jets see it: zero when every input sequence is final or the locktime is a height.
    pub fn lock_time(&self) -> u32 {
        self.read_lock(|locktime| locktime >= LOCK_TIME_THRESHOLD)
    }

    pub fn network(&self) -> &SimplicityNetwork {
        self.signer.get_network()
    }

    /// The signer that signs the transaction.
    pub fn signer(&self) -> &Signer {
        self.signer
    }

    /// The name of the program spending input `index`, `None` for a fixed or added input.
    pub(crate) fn spending_program(&self, index: usize) -> Option<&'a str> {
        self.scenario
            .program_scenario_by_input(index, &self.map)
            .map(|program_scenario| program_scenario.name())
    }

    /// Every output the lock check protects, with the name of the program it is locked to.
    pub(crate) fn locked_outputs(&self) -> Vec<(usize, &'a str)> {
        self.scenario.locked_outputs(&self.map)
    }

    /// Each program with its bound values, then the mutations.
    pub(crate) fn describe(&self) -> String {
        let mut description = self.scenario.describe(self.programs, self.transaction, &self.map);

        if !self.mutations.is_empty() {
            let mutations = self.mutations.iter().map(ToString::to_string).collect::<Vec<_>>();
            description += &format!("; mutations: {}", mutations.join(", "));
        }

        description
    }

    fn read_lock(&self, of_kind: impl Fn(u32) -> bool) -> u32 {
        let transaction = self
            .transaction
            .extract_pst()
            .0
            .extract_tx()
            .expect("the transaction has one kind of locktime");
        let locktime = transaction.lock_time.to_consensus_u32();
        let is_final = transaction.input.iter().all(|input| input.sequence.is_final());

        if !is_final && of_kind(locktime) { locktime } else { 0 }
    }
}
