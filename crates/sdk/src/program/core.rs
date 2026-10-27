use std::sync::{Arc, OnceLock};

use dyn_clone::DynClone;

use simplicityhl::ast::ElementsJetHinter;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::{Address, Script, Transaction, TxOut, taproot};
use simplicityhl::num::U256;
use simplicityhl::simplicity::bitcoin::{XOnlyPublicKey, secp256k1};
use simplicityhl::simplicity::jet::elements::{ElementsEnv, ElementsUtxo};
use simplicityhl::simplicity::{BitMachine, Cmr, RedeemNode, Value, leaf_version};
use simplicityhl::{Arguments, Parameters, ResolvedType, TemplateProgramWitness, WitnessTypes, WitnessValues};
use simplicityhl::{CompiledProgram, UnstableFeatures};

use crate::global::GlobalConfig;
use crate::program::logger::ProgramLogger;

use super::error::ProgramError;

use crate::provider::SimplicityNetwork;
use crate::utils::{check_budget, hash_script, tap_data_hash, tr_unspendable_key};

/// What the [`Signer`](crate::signer::Signer) needs from a program to sign for and finalize a
/// program input.
///
/// [`Program`] implements it, as does `&T` for any `T: ProgramTrait`.
/// [`ProgramInput`](crate::transaction::ProgramInput) holds a `Box<dyn ProgramTrait>`, which is
/// `Clone`.
pub trait ProgramTrait: DynClone + Send + Sync {
    /// Retrieves the types of arguments required by a `simplicity` program.
    ///
    /// # Errors
    /// Returns a `ProgramError` if parsing or generating ABI metadata fails.
    fn get_argument_types(&self) -> Result<Parameters, ProgramError>;

    /// Retrieves the witness types required by a `simplicity` program.
    ///
    /// # Errors
    /// Returns a `ProgramError` if parsing or generating ABI metadata fails.
    fn get_witness_types(&self) -> Result<WitnessTypes, ProgramError>;

    /// The Simplicity transaction environment for input `input_index` of `pst`: what the
    /// program's transaction jets, such as `jet::sig_all_hash()`, read.
    ///
    /// # Errors
    /// Returns `ProgramError::MissingWitnessUtxo` if an input of `pst` doesn't carry the output it
    /// spends, `ProgramError::UtxoIndexOutOfBounds` if `input_index` is past the inputs, and
    /// `ProgramError::ScriptPubkeyMismatch` if the input doesn't spend this program's address.
    fn get_env(
        &self,
        pst: &PartiallySignedTransaction,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<ElementsEnv<Arc<Transaction>>, ProgramError>;

    /// Runs the program locally on input `input_index` of `pst` with `witness`: satisfies it,
    /// prunes it, and runs the pruned program on the Bit Machine. Returns the pruned program and
    /// its output.
    ///
    /// Doesn't check the witness budget; [`ProgramTrait::finalize`] does. With [`Verbosity`](crate::global::Verbosity)
    /// `Debug` or `Trace`, the execution output is buffered, not printed: call
    /// [`ProgramLogger::flush_logs`] to print it.
    ///
    /// # Errors
    /// - `ProgramError::WitnessSatisfaction` if the witness doesn't fit the program's witness types.
    /// - `ProgramError::Pruning` if the program fails. A failed `assert!` or jet is
    ///   `ExecutionError::JetFailed`. Reaching a pruned branch, for example with `unwrap(None)`, is
    ///   `ExecutionError::ReachedPrunedBranch`. [`Program`] reports a failure in the Bit Machine run
    ///   after pruning with this variant too.
    /// - `ProgramError::BitMachineCreation` if the program exceeds the Bit Machine's limits.
    /// - The errors of [`ProgramTrait::get_env`] and of compiling the program.
    fn execute(
        &self,
        pst: &PartiallySignedTransaction,
        witness: &WitnessValues,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<(Arc<RedeemNode>, Value), ProgramError>;

    /// Runs the program as [`ProgramTrait::execute`] does and returns the input's witness stack:
    /// `[witness, program, cmr, control_block]`.
    ///
    /// Implementations must compare the pruned program's static cost with the budget from the
    /// exact serialized witness stack they return. Use [`check_budget`] for this check,
    /// as the built-in `Program` does.
    ///
    /// # Errors
    /// Returns the errors of [`ProgramTrait::execute`], and `ProgramError::InsufficientBudget` if the
    /// witness stack doesn't pay for the program's execution cost.
    fn finalize(
        &self,
        pst: &PartiallySignedTransaction,
        witness: &WitnessValues,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<Vec<Vec<u8>>, ProgramError>;
}

/// Static description of a program's inputs and integer literals. Implemented by `include_simf!()`.
///
/// The fuzz engine uses it to generate arguments and witnesses for a program it knows only by
/// type.
pub trait ProgramSchema {
    /// Generated arguments struct.
    type Arguments;

    /// Generated witness struct.
    type Witness;

    /// Parameter names and types.
    fn argument_types() -> Vec<(TemplateProgramWitness, ResolvedType)>;

    /// Witness names and types.
    fn witness_types() -> Vec<(TemplateProgramWitness, ResolvedType)>;

    /// The integer literals in the source, sorted and without duplicates.
    fn literals() -> Vec<U256>;
}

/// Creates a program of type `P` from arguments, for code that knows the program only by type,
/// such as the fuzz engine. Implemented by `include_simf!()`.
pub trait ProgramFactory<P: AsRef<Program> + Sized> {
    /// Instantiates a program with the given arguments.
    fn instantiate_program(args: impl Into<Arguments>) -> Box<P>;
}

/// A SimplicityHL program with its arguments, its Taproot internal key, and its storage slots.
///
/// The program compiles on first use, and the result is cached and shared between clones. The
/// getters that need the compiled program panic if it doesn't compile. Call [`Program::load`]
/// to get the compile error instead.
///
/// The program's address is a Taproot output. Its script tree holds the program as one leaf and
/// one hidden leaf per storage slot, `tagged_hash("TapData", slot)`. The tree is left-folded:
/// `tapbranch(tapbranch(tapbranch(program, slot0), slot1), slot2)`. So the source, the arguments
/// and the debug-symbol setting determine the CMR, and the address also depends on the internal
/// key and the storage.
///
/// Code generated by `simplex build` wraps a `Program` in one type per contract, which reaches it
/// with `as_ref()`.
#[derive(Clone)]
pub struct Program {
    source: Arc<str>,
    arguments: Arguments,
    pub_key: XOnlyPublicKey,
    storage: Vec<Vec<u8>>,
    include_debug_symbols: Option<bool>,
    compiled: Arc<OnceLock<(CompiledProgram, Cmr)>>,
    spend_info: OnceLock<taproot::TaprootSpendInfo>,
}

dyn_clone::clone_trait_object!(ProgramTrait);

impl ProgramTrait for Program {
    fn get_argument_types(&self) -> Result<Parameters, ProgramError> {
        self.get_argument_types()
    }

    fn get_witness_types(&self) -> Result<WitnessTypes, ProgramError> {
        self.get_witness_types()
    }

    fn get_env(
        &self,
        pst: &PartiallySignedTransaction,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<ElementsEnv<Arc<Transaction>>, ProgramError> {
        let genesis_hash = network.genesis_block_hash();
        let cmr = self.cmr()?;
        let utxos: Vec<TxOut> = pst
            .inputs()
            .iter()
            .enumerate()
            .map(|(input_index, input)| {
                input
                    .witness_utxo
                    .clone()
                    .ok_or(ProgramError::MissingWitnessUtxo { input_index })
            })
            .collect::<Result<_, _>>()?;

        if utxos.len() <= input_index {
            return Err(ProgramError::UtxoIndexOutOfBounds {
                input_index,
                utxo_count: utxos.len(),
            });
        }

        let target_utxo = &utxos[input_index];
        let script_pubkey = self.get_tr_address(network).script_pubkey();

        if target_utxo.script_pubkey != script_pubkey {
            return Err(ProgramError::ScriptPubkeyMismatch {
                expected_hash: script_pubkey.script_hash().to_string(),
                actual_hash: target_utxo.script_pubkey.script_hash().to_string(),
            });
        }

        Ok(ElementsEnv::new(
            Arc::new(pst.extract_tx()?),
            utxos
                .iter()
                .map(|utxo| ElementsUtxo {
                    script_pubkey: utxo.script_pubkey.clone(),
                    asset: utxo.asset,
                    value: utxo.value,
                })
                .collect(),
            u32::try_from(input_index)?,
            cmr,
            self.control_block()?,
            None,
            genesis_hash,
        ))
    }

    fn execute(
        &self,
        pst: &PartiallySignedTransaction,
        witness: &WitnessValues,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<(Arc<RedeemNode>, Value), ProgramError> {
        let satisfied = self
            .load()?
            .satisfy(witness.clone())
            .map_err(ProgramError::WitnessSatisfaction)?;

        // execute() is called multiple times during fee estimation; output is buffered
        // so only the final successful execution's logs are emitted to stderr.
        let mut tracker =
            ProgramLogger::make_tracker(input_index, satisfied.debug_symbols(), GlobalConfig::get_log_level());

        let env = self.get_env(pst, input_index, network)?;

        let pruned = satisfied.redeem().prune_with_tracker(&env, &mut tracker)?;

        if GlobalConfig::is_max_verbose() {
            ProgramLogger::buffer_cost_log(input_index, &pruned);
        }

        let mut mac = BitMachine::for_program(&pruned)?;

        let result = mac.exec(&pruned, &env)?;

        Ok((pruned, result))
    }

    fn finalize(
        &self,
        pst: &PartiallySignedTransaction,
        witness: &WitnessValues,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<Vec<Vec<u8>>, ProgramError> {
        let pruned = self.execute(pst, witness, input_index, network)?.0;

        let (simplicity_program_bytes, simplicity_witness_bytes) = pruned.to_vec_with_witness();
        let cmr = pruned.cmr();

        let stack = vec![
            simplicity_witness_bytes,
            simplicity_program_bytes,
            cmr.as_ref().to_vec(),
            self.control_block()?.serialize(),
        ];

        check_budget(pruned.bounds().cost, &stack)?;

        Ok(stack)
    }
}

impl<T: ProgramTrait + Sized> ProgramTrait for &T {
    fn get_argument_types(&self) -> Result<Parameters, ProgramError> {
        (*self).get_argument_types()
    }

    fn get_witness_types(&self) -> Result<WitnessTypes, ProgramError> {
        (*self).get_witness_types()
    }

    fn get_env(
        &self,
        pst: &PartiallySignedTransaction,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<ElementsEnv<Arc<Transaction>>, ProgramError> {
        (*self).get_env(pst, input_index, network)
    }

    fn execute(
        &self,
        pst: &PartiallySignedTransaction,
        witness: &WitnessValues,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<(Arc<RedeemNode>, Value), ProgramError> {
        (*self).execute(pst, witness, input_index, network)
    }

    fn finalize(
        &self,
        pst: &PartiallySignedTransaction,
        witness: &WitnessValues,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<Vec<Vec<u8>>, ProgramError> {
        (*self).finalize(pst, witness, input_index, network)
    }
}

impl Program {
    /// The width of a storage slot.
    pub const STORAGE_SLOT_BYTES: usize = 32;

    /// The most storage slots a program can have. Each slot deepens the Taproot tree by one, and
    /// the tree is at most 128 layers deep.
    pub const MAX_STORAGE_SLOTS: usize = 128;

    /// Creates a program from SimplicityHL source and its `param::` arguments, without compiling
    /// it.
    ///
    /// The Taproot internal key is [`tr_unspendable_key`], and there aren't any storage slots.
    #[must_use]
    pub fn new(source: impl Into<Arc<str>>, arguments: impl Into<Arguments>) -> Self {
        Self {
            source: source.into(),
            pub_key: tr_unspendable_key(),
            arguments: arguments.into(),
            storage: Vec::new(),
            include_debug_symbols: None,
            compiled: Arc::new(OnceLock::new()),
            spend_info: OnceLock::new(),
        }
    }

    /// Sets the Taproot internal key, which changes the address.
    #[must_use]
    pub fn with_taproot_pubkey(mut self, pub_key: XOnlyPublicKey) -> Self {
        self.pub_key = pub_key;
        self.spend_info = OnceLock::new();

        self
    }

    /// Compiles the program with or without debug symbols, overriding [`GlobalConfig`].
    ///
    /// Debug symbols change the CMR and so the address. A program whose address must match one
    /// computed elsewhere, such as a deployed contract, should set this explicitly, because
    /// verbosity `Debug` or `Trace` turns debug symbols on by default.
    #[must_use]
    pub fn with_debug_symbols(mut self, include: bool) -> Self {
        self.include_debug_symbols = Some(include);
        // This changes the output CMR, so we need to update the cache
        self.compiled = Arc::new(OnceLock::new());
        self.spend_info = OnceLock::new();

        self
    }

    /// Replaces the storage with `capacity` zeroed slots.
    ///
    /// # Panics
    /// Panics if `capacity` exceeds `MAX_STORAGE_SLOTS`.
    #[must_use]
    pub fn with_storage_capacity(mut self, capacity: usize) -> Self {
        Self::check_storage_capacity(capacity);

        self.storage = vec![vec![0u8; Self::STORAGE_SLOT_BYTES]; capacity];
        self.spend_info = OnceLock::new();

        self
    }

    /// Replaces the storage with `storage`, one value per slot.
    ///
    /// # Panics
    /// Panics if a value is not `STORAGE_SLOT_BYTES` wide, or if there are more than `MAX_STORAGE_SLOTS`.
    pub fn set_storage(&mut self, storage: impl IntoIterator<Item = impl Into<Vec<u8>>>) {
        let storage: Vec<Vec<u8>> = storage.into_iter().map(Into::into).collect();

        Self::check_storage_capacity(storage.len());

        self.storage = vec![vec![0u8; Self::STORAGE_SLOT_BYTES]; storage.len()];
        self.spend_info = OnceLock::new();

        for (index, value) in storage.into_iter().enumerate() {
            self.set_storage_at(index, value);
        }
    }

    /// Sets a 32-byte value at the specified index in the storage.
    ///
    /// # Panics
    /// Panics if the `index` is out of bounds for the initialized storage, or if the value is not
    /// `STORAGE_SLOT_BYTES` wide.
    pub fn set_storage_at(&mut self, index: usize, new_value: impl Into<Vec<u8>>) {
        let value = new_value.into();

        assert!(
            value.len() == Self::STORAGE_SLOT_BYTES,
            "A storage slot is {} bytes, and this one is {}",
            Self::STORAGE_SLOT_BYTES,
            value.len()
        );

        let slot = self.storage.get_mut(index).expect("Index out of bounds");

        *slot = value;
        self.spend_info = OnceLock::new();
    }

    /// Returns the arguments the program was created with.
    #[must_use]
    pub fn get_arguments(&self) -> &Arguments {
        &self.arguments
    }

    /// The number of storage slots.
    #[must_use]
    pub fn get_storage_len(&self) -> usize {
        self.storage.len()
    }

    /// The storage slots, each [`Program::STORAGE_SLOT_BYTES`] long.
    #[must_use]
    pub fn get_storage(&self) -> &[Vec<u8>] {
        &self.storage
    }

    /// The storage slot at `index`.
    ///
    /// # Panics
    /// Panics if the `index` is out of bounds for the initiated storage.
    #[must_use]
    pub fn get_storage_at(&self, index: usize) -> Vec<u8> {
        self.storage[index].clone()
    }

    /// The program's unconfidential Taproot address on `network`.
    ///
    /// # Panics
    /// Panics if the program doesn't compile.
    #[must_use]
    pub fn get_tr_address(&self, network: &SimplicityNetwork) -> Address {
        let spend_info = self.taproot_spending_info().unwrap_or_else(|error| panic!("{error}"));

        Address::p2tr(
            secp256k1::SECP256K1,
            spend_info.internal_key(),
            spend_info.merkle_root(),
            None,
            network.address_params(),
        )
    }

    /// The script of the program's address on `network`, which locks the outputs the program
    /// controls.
    ///
    /// # Panics
    /// Panics if the program doesn't compile.
    #[must_use]
    pub fn get_script_pubkey(&self, network: &SimplicityNetwork) -> Script {
        self.get_tr_address(network).script_pubkey()
    }

    /// The SHA-256 hash of [`Program::get_script_pubkey`], as Esplora's `scripthash` endpoints
    /// and the `jet::output_script_hash` family use.
    ///
    /// # Panics
    /// Panics if the program doesn't compile.
    #[must_use]
    pub fn get_script_hash(&self, network: &SimplicityNetwork) -> [u8; 32] {
        hash_script(&self.get_script_pubkey(network))
    }

    /// The program's Commitment Merkle Root.
    ///
    /// # Panics
    /// Panics if the `SimplicityHL` compilation fails.
    #[must_use]
    pub fn get_cmr(&self) -> [u8; 32] {
        self.cmr().unwrap_or_else(|error| panic!("{error}")).to_byte_array()
    }

    /// The tapleaf hash of the program's leaf. Storage doesn't change it.
    ///
    /// # Panics
    /// Panics if the `SimplicityHL` compilation fails.
    #[must_use]
    pub fn get_tapleaf_hash(&self) -> [u8; 32] {
        let (script, version) = self.script_version().unwrap_or_else(|error| panic!("{error}"));

        taproot::TapLeafHash::from_script(&script, version).to_byte_array()
    }

    /// The names and types of the program's `param::` arguments.
    ///
    /// # Errors
    /// Returns a `ProgramError` if compilation fails or generating ABI metadata fails.
    pub fn get_argument_types(&self) -> Result<Parameters, ProgramError> {
        let abi_meta = self
            .load()?
            .generate_abi_meta()
            .map_err(ProgramError::ProgramGenAbiMeta)?;

        Ok(abi_meta.param_types)
    }

    /// The names and types of the program's `witness::` fields.
    ///
    /// # Errors
    /// Returns a `ProgramError` if compilation fails.
    pub fn get_witness_types(&self) -> Result<WitnessTypes, ProgramError> {
        Ok(self.load()?.witness_types().shallow_clone())
    }

    /// Compiles the program, or returns the cached result of an earlier compilation.
    ///
    /// # Errors
    /// Returns `ProgramError::Compilation` if the source doesn't compile with these arguments.
    pub fn load(&self) -> Result<&CompiledProgram, ProgramError> {
        Ok(&self.load_with_cmr()?.0)
    }

    fn cmr(&self) -> Result<Cmr, ProgramError> {
        Ok(self.load_with_cmr()?.1)
    }

    fn load_with_cmr(&self) -> Result<&(CompiledProgram, Cmr), ProgramError> {
        // Check cache first
        if let Some(loaded) = self.compiled.get() {
            return Ok(loaded);
        }

        let compiled = CompiledProgram::new_with_unstable(
            Arc::clone(&self.source),
            &UnstableFeatures::all(),
            self.arguments.clone(),
            self.include_debug_symbols
                .unwrap_or_else(GlobalConfig::get_include_debug_symbols),
            Box::new(ElementsJetHinter),
        )
        .map_err(ProgramError::Compilation)?;
        let cmr = compiled.commit().cmr();

        // Update the cache
        Ok(self.compiled.get_or_init(|| (compiled, cmr)))
    }

    fn script_version(&self) -> Result<(Script, taproot::LeafVersion), ProgramError> {
        let cmr = self.cmr()?;
        let script = Script::from(cmr.as_ref().to_vec());

        Ok((script, leaf_version()))
    }

    fn taproot_leaf_depths(total_leaves: usize) -> Vec<usize> {
        assert!(total_leaves > 0, "Taproot tree must contain at least one leaf");

        let extra = total_leaves - 1;
        let mut depths = Vec::with_capacity(total_leaves);

        depths.push(extra);
        depths.extend((1..=extra).rev());

        depths
    }

    fn taproot_spending_info(&self) -> Result<&taproot::TaprootSpendInfo, ProgramError> {
        if let Some(spend_info) = self.spend_info.get() {
            return Ok(spend_info);
        }

        let mut builder = taproot::TaprootBuilder::new();
        let (script, version) = self.script_version()?;
        let depths = Self::taproot_leaf_depths(1 + self.get_storage_len());

        builder = builder
            .add_leaf_with_ver(depths[0], script, version)
            .expect("tap tree should be valid");

        for (slot, depth) in self.get_storage().iter().zip(depths.into_iter().skip(1)) {
            builder = builder
                .add_hidden(
                    depth,
                    taproot::TapNodeHash::from_byte_array(tap_data_hash(slot).to_byte_array()),
                )
                .expect("tap tree should be valid");
        }

        let spend_info = builder
            .finalize(secp256k1::SECP256K1, self.pub_key)
            .expect("tap tree should be valid");

        Ok(self.spend_info.get_or_init(|| spend_info))
    }

    fn control_block(&self) -> Result<taproot::ControlBlock, ProgramError> {
        let info = self.taproot_spending_info()?;
        let script_ver = self.script_version()?;

        Ok(info.control_block(&script_ver).expect("control block should exist"))
    }

    fn check_storage_capacity(capacity: usize) {
        assert!(
            capacity <= Self::MAX_STORAGE_SLOTS,
            "A program holds at most {} storage slots, and this one asks for {capacity}",
            Self::MAX_STORAGE_SLOTS
        );
    }
}

#[cfg(test)]
mod tests {
    use simplicityhl::{
        Arguments,
        elements::{AssetId, confidential, pset::Input},
    };

    use super::*;

    // simplicityhl/examples/cat.simf
    const DUMMY_PROGRAM: &str = r"
        fn main() {
            let ab: u16 = <(u8, u8)>::into((0x10, 0x01));
            let c: u16 = 0x1001;
            assert!(jet::eq_16(ab, c));
            let ab: u8 = <(u4, u4)>::into((0b1011, 0b1101));
            let c: u8 = 0b10111101;
            assert!(jet::eq_8(ab, c));
        }
    ";

    fn stateful(slots: usize) -> Program {
        Program::new(Arc::<str>::from(DUMMY_PROGRAM), &EmptyArguments).with_storage_capacity(slots)
    }

    #[test]
    fn storage_slot_takes_a_full_width_value() {
        let mut program = stateful(1);

        program.set_storage_at(0, vec![0x11; Program::STORAGE_SLOT_BYTES]);

        assert_eq!(program.get_storage_at(0), vec![0x11; Program::STORAGE_SLOT_BYTES]);
    }

    #[test]
    #[should_panic(expected = "A storage slot is 32 bytes, and this one is 1")]
    fn storage_slot_refuses_a_short_value() {
        stateful(1).set_storage_at(0, vec![0x11]);
    }

    #[test]
    #[should_panic(expected = "A storage slot is 32 bytes, and this one is 33")]
    fn storage_slot_refuses_a_long_one() {
        stateful(1).set_storage_at(0, vec![0x11; 33]);
    }

    #[test]
    fn changing_a_slot_changes_the_address_but_not_the_program() {
        let one = stateful(1);
        let network = SimplicityNetwork::LiquidTestnet;
        let address = one.get_tr_address(&network);

        let mut other = one.clone();
        other.set_storage_at(0, vec![0x01; Program::STORAGE_SLOT_BYTES]);

        assert_ne!(address, other.get_tr_address(&network));
        assert_eq!(address, one.get_tr_address(&network));
        assert_eq!(one.get_cmr(), other.get_cmr());
        assert_eq!(one.get_tapleaf_hash(), other.get_tapleaf_hash());
    }

    #[derive(Clone)]
    struct EmptyArguments;

    impl From<EmptyArguments> for Arguments {
        fn from(_val: EmptyArguments) -> Self {
            Arguments::default()
        }
    }

    impl From<&EmptyArguments> for Arguments {
        fn from(_val: &EmptyArguments) -> Self {
            Arguments::default()
        }
    }

    fn dummy_asset_id(byte: u8) -> AssetId {
        AssetId::from_byte_array([byte; 32])
    }

    fn dummy_program() -> Program {
        Program::new(DUMMY_PROGRAM, EmptyArguments {})
    }

    fn dummy_network() -> SimplicityNetwork {
        SimplicityNetwork::default_regtest()
    }

    fn make_pst_with_script(script: Script) -> PartiallySignedTransaction {
        let txout = TxOut {
            asset: confidential::Asset::Explicit(dummy_asset_id(0xAA)),
            value: confidential::Value::Explicit(1000),
            script_pubkey: script,
            ..Default::default()
        };
        let input = Input {
            witness_utxo: Some(txout),
            ..Default::default()
        };

        let mut pst = PartiallySignedTransaction::new_v2();

        pst.add_input(input);

        pst
    }

    #[test]
    fn compiles_once_and_keeps_it() {
        let program = dummy_program();

        let first = program.load().expect("the dummy program compiles");
        let second = program.load().expect("the dummy program compiles");

        assert!(std::ptr::eq(first, second), "the program was compiled twice");
    }

    #[test]
    fn changed_build_mode_is_not_served_the_old_compilation() {
        let plain = dummy_program();
        let plain_cmr = plain.get_cmr();

        let debug = dummy_program().with_debug_symbols(true);
        let debug_cmr = debug.get_cmr();

        assert_ne!(plain_cmr, debug_cmr, "the build mode did not reach the compiler");
    }

    #[test]
    fn test_get_env_idx() {
        let program = dummy_program();
        let network = dummy_network();

        let correct_script = program.get_script_pubkey(&network);
        let wrong_script = Script::new();

        let mut pst = make_pst_with_script(wrong_script);

        let correct_txout = TxOut {
            asset: confidential::Asset::Explicit(dummy_asset_id(0xAA)),
            value: confidential::Value::Explicit(1000),
            script_pubkey: correct_script,
            ..Default::default()
        };

        pst.add_input(Input {
            witness_utxo: Some(correct_txout),
            ..Default::default()
        });

        // take a script with a wrong pubkey
        assert!(matches!(
            program.get_env(&pst, 0, &network).unwrap_err(),
            ProgramError::ScriptPubkeyMismatch { .. }
        ));

        assert!(program.get_env(&pst, 1, &network).is_ok());
    }

    #[test]
    fn input_without_its_utxo_is_an_error() {
        let network = SimplicityNetwork::default_regtest();
        let program = stateful(0);
        let mut pst = PartiallySignedTransaction::new_v2();

        pst.add_input(Input::default());
        pst.add_input(Input {
            witness_utxo: Some(TxOut {
                script_pubkey: program.get_script_pubkey(&network),
                ..Default::default()
            }),
            ..Default::default()
        });

        assert!(matches!(
            program.get_env(&pst, 1, &network).unwrap_err(),
            ProgramError::MissingWitnessUtxo { input_index: 0 }
        ));
    }

    #[test]
    fn storage_is_at_most_as_deep_as_the_taproot_tree() {
        let network = SimplicityNetwork::default_regtest();

        let _ = stateful(Program::MAX_STORAGE_SLOTS).get_tr_address(&network);
    }

    #[test]
    #[should_panic(expected = "A program holds at most 128 storage slots, and this one asks for 129")]
    fn storage_beyond_the_taproot_tree_is_refused() {
        let _ = stateful(Program::MAX_STORAGE_SLOTS + 1);
    }

    #[test]
    fn left_folded_depths() {
        assert_eq!(Program::taproot_leaf_depths(1), vec![0]);
        assert_eq!(Program::taproot_leaf_depths(2), vec![1, 1]);
        assert_eq!(Program::taproot_leaf_depths(3), vec![2, 2, 1]);
        assert_eq!(Program::taproot_leaf_depths(4), vec![3, 3, 2, 1]);
        assert_eq!(Program::taproot_leaf_depths(5), vec![4, 4, 3, 2, 1]);
    }

    // The measured boundary: a balanced tree agrees with a left fold up to three leaves and
    // diverges from four. Four leaves balanced is [2, 2, 2, 2]; left-folded it is not.
    #[test]
    fn diverges_from_a_balanced_tree_at_four_leaves() {
        assert_eq!(Program::taproot_leaf_depths(3), vec![2, 2, 1]);
        assert_ne!(Program::taproot_leaf_depths(4), vec![2, 2, 2, 2]);
    }
}
