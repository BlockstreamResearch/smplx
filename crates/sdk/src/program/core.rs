use std::collections::HashMap;
use std::iter;
use std::sync::{Arc, OnceLock};

use dyn_clone::DynClone;

use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::{Address, Script, Transaction, TxOut, taproot};
use simplicityhl::parse::ParseFromStr;
use simplicityhl::simplicity::bitcoin::{XOnlyPublicKey, secp256k1};
use simplicityhl::simplicity::jet::elements::{ElementsEnv, ElementsUtxo};
use simplicityhl::simplicity::{BitMachine, Cmr, RedeemNode, Value, leaf_version};
use simplicityhl::str::WitnessName;
use simplicityhl::types::ResolvedType;
use simplicityhl::{Parameters, WitnessTypes, WitnessValues};

use crate::compiler;
use crate::global::GlobalConfig;
use crate::program::logger::ProgramLogger;

use super::arguments::ArgumentsTrait;
use super::artifact::Artifact;
use super::error::ProgramError;

use crate::provider::SimplicityNetwork;
use crate::utils::{hash_script, tap_data_hash, tr_unspendable_key};

/// Executes `simplicity` programs at runtime.
///
/// This trait defines a core behavior related to testing and execution.
pub trait ProgramTrait: DynClone {
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

    /// Constructs the Elements environment for a specified input index, PST, and network for further program execution.
    ///
    /// # Errors
    /// Returns a `ProgramError` if the input index is out of bounds or if the script pubkey of the UTXO mismatches the expected program script.
    fn get_env(
        &self,
        pst: &PartiallySignedTransaction,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<ElementsEnv<Arc<Transaction>>, ProgramError>;

    /// Executes a Simplicity program for the given input index of a partially signed transaction.
    ///
    /// This function evaluates a Simplicity script associated with a specific transaction input
    /// in a given network, producing the result of the computation along with the redeem node
    /// used during execution.
    ///
    /// # Errors
    /// Returns a `ProgramError` if loading the program, satisfying the witness, retrieving the environment, or executing the `BitMachine` fails.
    fn execute(
        &self,
        pst: &PartiallySignedTransaction,
        witness: &WitnessValues,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<(Arc<RedeemNode>, Value), ProgramError>;

    /// Finalizes and returns `pruned_witness` as output after executing the program on certain parameters.
    ///
    /// # Errors
    /// Returns a `ProgramError` if program execution or constructing the control block fails.
    fn finalize(
        &self,
        pst: &PartiallySignedTransaction,
        witness: &WitnessValues,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<Vec<Vec<u8>>, ProgramError>;
}

/// Represents a program structure containing its source, a public key, arguments, and associated storage.
///
/// Abstraction giving the power to execute Simplicity contracts without specifying any additional parameters.
///
/// The program's Simplicity — and thus its CMR, address and spend — is produced by
/// an out-of-process `simc` pinned to a version, not the linked frontend.
/// Compilation is lazy and cached across clones.
#[derive(Clone)]
pub struct Program {
    /// `(relative-path, contents)` source files: the entry, everything its imports
    /// reach, and dependency files under their embedded slots.
    sources: Arc<[(String, String)]>,
    /// Relative path of the entry file within [`sources`](Self::sources).
    entry: String,
    /// Dependency remappings resolving `use <alias>::…` imports.
    deps: Arc<[(String, String, String)]>,
    pub_key: XOnlyPublicKey,
    arguments: Box<dyn ArgumentsTrait>,
    storage: Vec<[u8; 32]>,
    /// `None` resolves lazily from the environment or the nearest `simplex.lock` —
    /// never from what happens to be installed.
    compiler_version: Option<String>,
    /// Version-stable ABI baked in by the build tool. `None` when constructed
    /// without it; `Some(Err)` when the JSON was malformed, surfaced on first use.
    abi: Option<Result<Arc<AbiJson>, String>>,
    /// Lazily compiled artifact, shared across clones so the compiler runs at most once.
    compiled: Arc<OnceLock<Artifact>>,
}

/// The build tool's ABI record: `name -> type-string` maps.
#[derive(Clone, serde::Deserialize)]
struct AbiJson {
    #[serde(default)]
    witness_types: HashMap<String, String>,
    #[serde(default)]
    parameter_types: HashMap<String, String>,
}

impl AbiJson {
    /// Reconstructs a `name -> ResolvedType` map from `name -> type-string`.
    fn resolve(map: &HashMap<String, String>) -> Result<HashMap<WitnessName, ResolvedType>, ProgramError> {
        map.iter()
            .map(|(name, ty)| {
                let resolved = ResolvedType::parse_from_str(ty)
                    .map_err(|e| ProgramError::ProgramGenAbiMeta(format!("type `{ty}`: {e}")))?;
                Ok((WitnessName::from_str_unchecked(name), resolved))
            })
            .collect()
    }
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
        let cmr = self.commitment_cmr()?;
        let utxos: Vec<TxOut> = pst.inputs().iter().filter_map(|x| x.witness_utxo.clone()).collect();

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
        let artifact = self.artifact()?;
        let env = self.get_env(pst, input_index, network)?;

        // When the ABI is baked in, a wrong-typed witness fails here naming its
        // declared type; a malformed ABI is surfaced, not silently skipped.
        match &self.abi {
            Some(Ok(abi)) => {
                let types = WitnessTypes::from(AbiJson::resolve(&abi.witness_types)?);
                witness
                    .is_consistent(&types)
                    .map_err(|e| ProgramError::WitnessSatisfaction(e.to_string()))?;
            }
            Some(Err(_)) => {
                self.require_abi()?;
            }
            None => {}
        }

        let redeem = artifact.satisfy(witness).map_err(ProgramError::WitnessSatisfaction)?;

        // A debug artifact's symbols drive the tracker. Output is buffered so only
        // the final execution's logs reach stderr (fee estimation runs this repeatedly).
        let pruned = match artifact.debug_symbols() {
            Some(symbols) => {
                let mut tracker = ProgramLogger::make_tracker(symbols, GlobalConfig::get_log_level());
                redeem.prune_with_tracker(&env, &mut tracker)?
            }
            None => redeem.prune(&env)?,
        };

        if GlobalConfig::is_max_verbose() {
            ProgramLogger::buffer_cost_log(&pruned);
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

        Ok(vec![
            simplicity_witness_bytes,
            simplicity_program_bytes,
            cmr.as_ref().to_vec(),
            self.control_block()?.serialize(),
        ])
    }
}

impl Program {
    /// Creates a program from a single self-contained source string; for multi-file
    /// programs use [`Self::from_sources`].
    #[must_use]
    pub fn new(source: &'static str, arguments: Box<dyn ArgumentsTrait>) -> Self {
        Self::from_sources(&[("contract.simf", source)], "contract.simf", arguments)
    }

    /// Creates a program from `(relative-path, contents)` sources and the relative
    /// path of the entry file among them.
    #[must_use]
    pub fn from_sources(sources: &[(&str, &str)], entry: &str, arguments: Box<dyn ArgumentsTrait>) -> Self {
        let sources: Vec<(String, String)> = sources
            .iter()
            .map(|(path, contents)| ((*path).to_string(), (*contents).to_string()))
            .collect();
        Self {
            sources: Arc::from(sources),
            entry: entry.to_string(),
            deps: Arc::from(Vec::new()),
            pub_key: tr_unspendable_key(),
            arguments,
            storage: Vec::new(),
            compiler_version: None,
            abi: None,
            compiled: Arc::new(OnceLock::new()),
        }
    }

    /// Attaches the build's dependency remappings: `(context, alias, target)` paths
    /// relative to the materialized source root (`""` is the root).
    #[must_use]
    pub fn with_deps(mut self, deps: &[(&str, &str, &str)]) -> Self {
        let deps: Vec<(String, String, String)> = deps
            .iter()
            .map(|(context, alias, target)| ((*context).to_string(), (*alias).to_string(), (*target).to_string()))
            .collect();
        self.deps = Arc::from(deps);
        self
    }

    /// Pins the compiler version that produces this program's CMR and spend.
    #[must_use]
    pub fn with_compiler_version(mut self, version: impl Into<String>) -> Self {
        self.compiler_version = Some(version.into());
        self
    }

    /// Attaches the ABI emitted by the build tool, backing the type accessors
    /// without a compiler. Malformed JSON is kept and reported on first use.
    #[must_use]
    pub fn with_abi_json(mut self, abi_json: &str) -> Self {
        self.abi = Some(
            serde_json::from_str::<AbiJson>(abi_json)
                .map(Arc::new)
                .map_err(|e| e.to_string()),
        );
        self
    }

    /// Sets the `pub_key` field of the struct to the provided `XOnlyPublicKey` value and returns the updated builder instance.
    /// This is used to set the taproot public key for the program.
    #[must_use]
    pub fn with_taproot_pubkey(mut self, pub_key: XOnlyPublicKey) -> Self {
        self.pub_key = pub_key;

        self
    }

    /// Sets storage capacity for further usage.
    #[must_use]
    pub fn with_storage_capacity(mut self, capacity: usize) -> Self {
        self.storage = vec![[0u8; 32]; capacity];

        self
    }

    /// Sets a 32-byte value at the specified index in the storage.
    ///
    /// # Panics
    /// Panics if the `index` is out of bounds for the initiasized storage.
    pub fn set_storage_at(&mut self, index: usize, new_value: [u8; 32]) {
        let slot = self.storage.get_mut(index).expect("Index out of bounds");

        *slot = new_value;
    }

    /// Returns the number of storage chunks for a program.
    #[must_use]
    pub fn get_storage_len(&self) -> usize {
        self.storage.len()
    }

    /// Returns storage as a whole array of 32-byte chunks.
    #[must_use]
    pub fn get_storage(&self) -> &[[u8; 32]] {
        &self.storage
    }

    /// Returns storage value at a certain index.
    ///
    /// # Panics
    /// Panics if the `index` is out of bounds for the initiated storage.
    #[must_use]
    pub fn get_storage_at(&self, index: usize) -> [u8; 32] {
        self.storage[index]
    }

    /// Returns a taproot address for a defined `SimplicityNetwork`.
    ///
    /// # Panics
    /// Panics if generating the taproot spending information fails, or when deriving
    /// a mainnet address while debug verbosity is on (instrumentation changes the CMR).
    #[must_use]
    pub fn get_tr_address(&self, network: &SimplicityNetwork) -> Address {
        Self::guard_debug_address(GlobalConfig::get_include_debug_symbols(), network);
        let spend_info = self.taproot_spending_info().unwrap();

        Address::p2tr(
            secp256k1::SECP256K1,
            spend_info.internal_key(),
            spend_info.merkle_root(),
            None,
            network.address_params(),
        )
    }

    /// Debug instrumentation changes the CMR. On regtest that is self-consistent; on
    /// mainnet it must never escape — funds would go to an address nobody derives
    /// without `-v`.
    fn guard_debug_address(debug: bool, network: &SimplicityNetwork) {
        if !debug {
            return;
        }
        match network {
            SimplicityNetwork::Liquid => panic!(
                "refusing to derive a mainnet address from a debug-instrumented program \
                 (-v/-vv changes the CMR); disable verbosity for mainnet use"
            ),
            SimplicityNetwork::LiquidTestnet => eprintln!(
                "warning: deriving a testnet address from a debug-instrumented program; \
                 it differs from the address of a non-verbose build"
            ),
            SimplicityNetwork::ElementsRegtest { .. } => {}
        }
    }

    /// Retrieves the `ScriptPubKey` associated with the Simplicity address for the specified network.
    #[must_use]
    pub fn get_script_pubkey(&self, network: &SimplicityNetwork) -> Script {
        self.get_tr_address(network).script_pubkey()
    }

    /// Retrieves the 32-byte `ScriptPubKey` hash associated with the Simplicity address for the specified network.
    #[must_use]
    pub fn get_script_hash(&self, network: &SimplicityNetwork) -> [u8; 32] {
        hash_script(&self.get_script_pubkey(network))
    }

    /// Retrieves program ABI metadata for argument types.
    ///
    /// # Errors
    /// Returns a `ProgramError` if the ABI was not baked in at build time or a
    /// declared type cannot be parsed.
    pub fn get_argument_types(&self) -> Result<Parameters, ProgramError> {
        let abi = self.require_abi()?;
        Ok(Parameters::from(AbiJson::resolve(&abi.parameter_types)?))
    }

    /// Retrieves the witness types from the program's ABI metadata.
    ///
    /// # Errors
    /// Returns a `ProgramError` if the ABI was not baked in at build time or a
    /// declared type cannot be parsed.
    pub fn get_witness_types(&self) -> Result<WitnessTypes, ProgramError> {
        let abi = self.require_abi()?;
        Ok(WitnessTypes::from(AbiJson::resolve(&abi.witness_types)?))
    }

    fn require_abi(&self) -> Result<&AbiJson, ProgramError> {
        match &self.abi {
            Some(Ok(abi)) => Ok(abi),
            Some(Err(parse_error)) => Err(ProgramError::ProgramGenAbiMeta(format!(
                "ABI metadata is malformed ({parse_error}); regenerate bindings with `simplex build`"
            ))),
            None => Err(ProgramError::ProgramGenAbiMeta(
                "ABI metadata is unavailable; regenerate bindings with `simplex build`".to_string(),
            )),
        }
    }

    /// The compiled artifact, produced by the pinned `simc` on first use and cached.
    fn artifact(&self) -> Result<&Artifact, ProgramError> {
        if let Some(artifact) = self.compiled.get() {
            return Ok(artifact);
        }

        let version = match &self.compiler_version {
            Some(version) => version.clone(),
            None => compiler::resolve_version().map_err(|e| ProgramError::Compilation(e.to_string()))?,
        };
        let simc = compiler::ensure(&version).map_err(|e| ProgramError::Compilation(e.to_string()))?;
        let artifact = compiler::compile(
            &simc,
            &self.sources,
            &self.entry,
            &self.deps,
            &self.arguments.build_arguments(),
            // A verbose run derives and spends the instrumented program, consistently.
            GlobalConfig::get_include_debug_symbols(),
        )
        .map_err(|e| ProgramError::Compilation(e.to_string()))?;

        // If another thread won the race, keep its value; the result is deterministic.
        let _ = self.compiled.set(artifact);
        Ok(self.compiled.get().expect("artifact was just set"))
    }

    fn commitment_cmr(&self) -> Result<Cmr, ProgramError> {
        Ok(self.artifact()?.cmr())
    }

    fn script_version(&self) -> Result<(Script, taproot::LeafVersion), ProgramError> {
        let cmr = self.commitment_cmr()?;
        let script = Script::from(cmr.as_ref().to_vec());

        Ok((script, leaf_version()))
    }

    fn taproot_leaf_depths(total_leaves: usize) -> Vec<usize> {
        assert!(total_leaves > 0, "Taproot tree must contain at least one leaf");

        let next_pow2 = total_leaves.next_power_of_two();
        let depth = next_pow2.ilog2() as usize;

        let shallow_count = next_pow2 - total_leaves;
        let deep_count = total_leaves - shallow_count;

        let mut depths = Vec::with_capacity(total_leaves);
        depths.extend(iter::repeat_n(depth, deep_count));

        if depth > 0 {
            depths.extend(iter::repeat_n(depth - 1, shallow_count));
        }

        depths
    }

    fn taproot_spending_info(&self) -> Result<taproot::TaprootSpendInfo, ProgramError> {
        let mut builder = taproot::TaprootBuilder::new();
        let (script, version) = self.script_version()?;
        let depths = Self::taproot_leaf_depths(1 + self.get_storage_len());

        builder = builder
            .add_leaf_with_ver(depths[0], script, version)
            .expect("tap tree should be valid");

        for (slot, depth) in self.get_storage().iter().zip(depths.into_iter().skip(1)) {
            builder = builder
                .add_hidden(depth, tap_data_hash(slot))
                .expect("tap tree should be valid");
        }

        Ok(builder
            .finalize(secp256k1::SECP256K1, self.pub_key)
            .expect("tap tree should be valid"))
    }

    fn control_block(&self) -> Result<taproot::ControlBlock, ProgramError> {
        let info = self.taproot_spending_info()?;
        let script_ver = self.script_version()?;

        Ok(info.control_block(&script_ver).expect("control block should exist"))
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

    #[derive(Clone)]
    struct EmptyArguments;

    impl ArgumentsTrait for EmptyArguments {
        fn build_arguments(&self) -> Arguments {
            Arguments::default()
        }
    }

    /// The compiler version matching the linked frontend, if that exact `simc` is in
    /// the store. Tests that need to actually compile skip when it is absent, so the
    /// default `cargo test` stays hermetic; provision with `simplex build`/`toolchain`.
    fn provisioned_matching_version() -> Option<String> {
        let version = simplicityhl::version::SimcDirective::current_version().to_string();
        let simc = crate::compiler::simc_path(&version).ok()?;
        simc.is_file().then_some(version)
    }

    fn dummy_asset_id(byte: u8) -> AssetId {
        AssetId::from_slice(&[byte; 32]).unwrap()
    }

    fn dummy_program() -> Program {
        Program::new(DUMMY_PROGRAM, Box::new(EmptyArguments))
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
    fn test_get_env_idx() {
        let Some(version) = provisioned_matching_version() else {
            eprintln!("skipping: no provisioned simc for the linked compiler version");
            return;
        };
        let program = dummy_program().with_compiler_version(version);
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
    fn test_taproot_leaf_depths_known_values() {
        let cases = [
            (1, vec![0]),
            (2, vec![1, 1]),
            (3, vec![2, 2, 1]),
            (4, vec![2, 2, 2, 2]),
            (5, vec![3, 3, 2, 2, 2]),
            (6, vec![3, 3, 3, 3, 2, 2]),
            (8, vec![3, 3, 3, 3, 3, 3, 3, 3]),
        ];

        for (n, expected) in cases {
            assert_eq!(Program::taproot_leaf_depths(n), expected, "n={n}");
        }
    }

    /// A witness value `A = v`, built the way generated `*Witness` structs do.
    fn witness_a(value: u16) -> WitnessValues {
        use simplicityhl::value::UIntValue;

        let mut map = std::collections::HashMap::new();
        map.insert(
            WitnessName::from_str_unchecked("A"),
            simplicityhl::Value::from(UIntValue::U16(value)),
        );
        WitnessValues::from(map)
    }

    #[test]
    fn out_of_process_program_matches_linked_cmr() {
        use simplicityhl::ast::ElementsJetHinter;
        use simplicityhl::{CompiledProgram, UnstableFeatures};

        const SRC: &str = "fn main() { let a: u16 = witness::A; assert!(jet::eq_16(a, 7)); }";

        let Some(version) = provisioned_matching_version() else {
            eprintln!("skipping: no provisioned simc for the linked compiler version");
            return;
        };

        let network = dummy_network();

        // Ground truth CMR from the linked frontend.
        let linked = CompiledProgram::new_with_unstable(
            SRC,
            &UnstableFeatures::all(),
            Arguments::default(),
            false,
            Box::new(ElementsJetHinter),
        )
        .unwrap();
        let expected_cmr = linked.commit().cmr();

        // Out-of-process program: CMR comes from the pinned simc.
        let program = Program::new(SRC, Box::new(EmptyArguments)).with_compiler_version(version);
        assert_eq!(
            program.commitment_cmr().unwrap(),
            expected_cmr,
            "out-of-process CMR must match linked"
        );

        // And the full spend finalizes through the artifact satisfier.
        let script = program.get_script_pubkey(&network);
        let pst = make_pst_with_script(script);
        let finalized = program.finalize(&pst, &witness_a(7), 0, &network).unwrap();
        assert_eq!(
            finalized.len(),
            4,
            "finalize returns witness, program, cmr, control block"
        );
    }

    #[test]
    #[should_panic(expected = "refusing to derive a mainnet address")]
    fn debug_address_refused_on_mainnet() {
        Program::guard_debug_address(true, &SimplicityNetwork::Liquid);
    }

    #[test]
    fn debug_address_allowed_off_mainnet_or_without_debug() {
        Program::guard_debug_address(false, &SimplicityNetwork::Liquid);
        Program::guard_debug_address(true, &SimplicityNetwork::default_regtest());
    }
}
