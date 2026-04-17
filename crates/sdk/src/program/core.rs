use std::iter;
use std::sync::Arc;

use dyn_clone::DynClone;

use simplicityhl::CompiledProgram;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::{Address, Script, Transaction, TxOut, taproot};
use simplicityhl::simplicity::bitcoin::{XOnlyPublicKey, secp256k1};
use simplicityhl::simplicity::jet::Elements;
use simplicityhl::simplicity::jet::elements::{ElementsEnv, ElementsUtxo};
use simplicityhl::simplicity::{BitMachine, RedeemNode, Value, leaf_version};
use simplicityhl::tracker::{DefaultTracker, TrackerLogLevel};
use simplicityhl::{Parameters, WitnessTypes, WitnessValues};

use super::arguments::ArgumentsTrait;
use super::error::ProgramError;

use crate::provider::SimplicityNetwork;
use crate::utils::{hash_script, tap_data_hash, tr_unspendable_key};

pub trait ProgramTrait: DynClone {
    fn get_argument_types(&self) -> Result<Parameters, ProgramError>;

    fn get_witness_types(&self) -> Result<WitnessTypes, ProgramError>;

    fn get_env(
        &self,
        pst: &PartiallySignedTransaction,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<ElementsEnv<Arc<Transaction>>, ProgramError>;

    fn execute(
        &self,
        pst: &PartiallySignedTransaction,
        witness: &WitnessValues,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<(Arc<RedeemNode<Elements>>, Value), ProgramError>;

    fn finalize(
        &self,
        pst: &PartiallySignedTransaction,
        witness: &WitnessValues,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<Vec<Vec<u8>>, ProgramError>;
}

#[derive(Clone)]
pub struct Program {
    source: &'static str,
    pub_key: XOnlyPublicKey,
    arguments: Box<dyn ArgumentsTrait>,
    storage: Vec<[u8; 32]>,
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
        let cmr = self.load()?.commit().cmr();
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
    ) -> Result<(Arc<RedeemNode<Elements>>, Value), ProgramError> {
        let satisfied = self
            .load()?
            .satisfy(witness.clone())
            .map_err(ProgramError::WitnessSatisfaction)?;

        // TODO: global config for TrackerLogLevel
        let mut tracker = DefaultTracker::new(satisfied.debug_symbols()).with_log_level(TrackerLogLevel::Debug);

        let env = self.get_env(pst, input_index, network)?;

        let pruned = satisfied.redeem().prune_with_tracker(&env, &mut tracker)?;
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
    pub fn new(source: &'static str, arguments: Box<dyn ArgumentsTrait>) -> Self {
        Self {
            source,
            pub_key: tr_unspendable_key(),
            arguments,
            storage: Vec::new(),
        }
    }

    pub fn with_pub_key(mut self, pub_key: XOnlyPublicKey) -> Self {
        self.pub_key = pub_key;

        self
    }

    pub fn with_storage_capacity(mut self, capacity: usize) -> Self {
        self.storage = vec![[0u8; 32]; capacity];

        self
    }

    pub fn set_storage_at(&mut self, index: usize, new_value: [u8; 32]) {
        let slot = self.storage.get_mut(index).expect("Index out of bounds");

        *slot = new_value;
    }

    pub fn get_storage_len(&self) -> usize {
        self.storage.len()
    }

    pub fn get_storage(&self) -> &[[u8; 32]] {
        &self.storage
    }

    pub fn get_storage_at(&self, index: usize) -> [u8; 32] {
        self.storage[index]
    }

    pub fn get_tr_address(&self, network: &SimplicityNetwork) -> Address {
        let spend_info = self.taproot_spending_info().unwrap();

        Address::p2tr(
            secp256k1::SECP256K1,
            spend_info.internal_key(),
            spend_info.merkle_root(),
            None,
            network.address_params(),
        )
    }

    pub fn get_script_pubkey(&self, network: &SimplicityNetwork) -> Script {
        self.get_tr_address(network).script_pubkey()
    }

    pub fn get_script_hash(&self, network: &SimplicityNetwork) -> [u8; 32] {
        hash_script(&self.get_script_pubkey(network))
    }

    pub fn get_argument_types(&self) -> Result<Parameters, ProgramError> {
        let compiled = self.load()?;
        let abi_meta = compiled.generate_abi_meta().map_err(ProgramError::ProgramGenAbiMeta)?;

        Ok(abi_meta.param_types)
    }

    pub fn get_witness_types(&self) -> Result<WitnessTypes, ProgramError> {
        let compiled = self.load()?;
        let abi_meta = compiled.generate_abi_meta().map_err(ProgramError::ProgramGenAbiMeta)?;

        Ok(abi_meta.witness_types)
    }

    fn load(&self) -> Result<CompiledProgram, ProgramError> {
        let compiled = CompiledProgram::new(self.source, self.arguments.build_arguments(), true)
            .map_err(ProgramError::Compilation)?;
        Ok(compiled)
    }

    fn script_version(&self) -> Result<(Script, taproot::LeafVersion), ProgramError> {
        let cmr = self.load()?.commit().cmr();
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
    const DUMMY_PROGRAM: &str = r#"
        fn main() {
            let ab: u16 = <(u8, u8)>::into((0x10, 0x01));
            let c: u16 = 0x1001;
            assert!(jet::eq_16(ab, c));
            let ab: u8 = <(u4, u4)>::into((0b1011, 0b1101));
            let c: u8 = 0b10111101;
            assert!(jet::eq_8(ab, c));
        }
    "#;

    #[derive(Clone)]
    struct EmptyArguments;

    impl ArgumentsTrait for EmptyArguments {
        fn build_arguments(&self) -> Arguments {
            Arguments::default()
        }
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
}
