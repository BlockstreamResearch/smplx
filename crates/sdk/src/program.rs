use std::sync::Arc;

use sha2::{Digest, Sha256};

use simplicityhl::CompiledProgram;
use simplicityhl::elements::{Address, Script, Transaction, TxInWitness, TxOut, script, taproot};
use simplicityhl::simplicity::bitcoin::{XOnlyPublicKey, secp256k1};
use simplicityhl::simplicity::jet::Elements;
use simplicityhl::simplicity::jet::elements::{ElementsEnv, ElementsUtxo};
use simplicityhl::simplicity::{BitMachine, RedeemNode, Value};
use simplicityhl::tracker::{DefaultTracker, TrackerLogLevel};
use simplicityhl::{Arguments, WitnessValues};

use crate::constants::SimplicityNetwork;
use crate::error::ProgramError;

pub trait ArgumentsTrait {
    fn build_arguments(&self) -> Arguments;
}

pub trait WitnessTrait {
    fn build_witness(&self) -> WitnessValues;
}

pub trait ProgramTrait {
    fn get_env(
        &self,
        tx: &Transaction,
        utxos: &[TxOut],
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<ElementsEnv<Arc<Transaction>>, ProgramError>;

    fn execute(
        &self,
        witness: WitnessValues,
        tx: &Transaction,
        utxos: &[TxOut],
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<(Arc<RedeemNode<Elements>>, Value), ProgramError>;

    fn finalize(
        &self,
        witness: WitnessValues,
        tx: Transaction,
        utxos: &[TxOut],
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<Transaction, ProgramError>;
}

pub struct Program<'a> {
    source: &'static str,
    pub_key: &'a XOnlyPublicKey,
    arguments: &'a dyn ArgumentsTrait,
}

impl<'a> ProgramTrait for Program<'a> {
    fn get_env(
        &self,
        tx: &Transaction,
        utxos: &[TxOut],
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<ElementsEnv<Arc<Transaction>>, ProgramError> {
        let genesis_hash = network.genesis_block_hash();
        let cmr = self.load()?.commit().cmr();

        if utxos.len() <= input_index {
            return Err(ProgramError::UtxoIndexOutOfBounds {
                input_index,
                utxo_count: utxos.len(),
            });
        }

        let target_utxo = &utxos[input_index];
        let script_pubkey = self.get_tr_address(network)?.script_pubkey();

        if target_utxo.script_pubkey != script_pubkey {
            return Err(ProgramError::ScriptPubkeyMismatch {
                expected_hash: script_pubkey.script_hash().to_string(),
                actual_hash: target_utxo.script_pubkey.script_hash().to_string(),
            });
        }

        Ok(ElementsEnv::new(
            Arc::new(tx.clone()),
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
        witness: WitnessValues,
        tx: &Transaction,
        utxos: &[TxOut],
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<(Arc<RedeemNode<Elements>>, Value), ProgramError> {
        let satisfied = self
            .load()?
            .satisfy(witness)
            .map_err(ProgramError::WitnessSatisfaction)?;

        let mut tracker = DefaultTracker::new(satisfied.debug_symbols()).with_log_level(TrackerLogLevel::Debug);

        let env = self.get_env(tx, utxos, input_index, network)?;

        let pruned = satisfied.redeem().prune_with_tracker(&env, &mut tracker)?;
        let mut mac = BitMachine::for_program(&pruned)?;

        let result = mac.exec(&pruned, &env).map_err(ProgramError::Execution)?;

        Ok((pruned, result))
    }

    fn finalize(
        &self,
        witness: WitnessValues,
        mut tx: Transaction,
        utxos: &[TxOut],
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<Transaction, ProgramError> {
        let pruned = self.execute(witness, &tx, utxos, input_index, network)?.0;

        let (simplicity_program_bytes, simplicity_witness_bytes) = pruned.to_vec_with_witness();
        let cmr = pruned.cmr();

        tx.input[input_index].witness = TxInWitness {
            amount_rangeproof: None,
            inflation_keys_rangeproof: None,
            script_witness: vec![
                simplicity_witness_bytes,
                simplicity_program_bytes,
                cmr.as_ref().to_vec(),
                self.control_block()?.serialize(),
            ],
            pegin_witness: vec![],
        };

        Ok(tx)
    }
}

impl<'a> Program<'a> {
    pub fn new(source: &'static str, pub_key: &'a XOnlyPublicKey, arguments: &'a impl ArgumentsTrait) -> Self {
        Self {
            source: source,
            pub_key: pub_key,
            arguments: arguments,
        }
    }

    pub fn get_tr_address(&self, network: SimplicityNetwork) -> Result<Address, ProgramError> {
        let spend_info = self.taproot_spending_info()?;

        Ok(Address::p2tr(
            secp256k1::SECP256K1,
            spend_info.internal_key(),
            spend_info.merkle_root(),
            None,
            network.address_params(),
        ))
    }

    pub fn get_script_pubkey(&self, network: SimplicityNetwork) -> Result<Script, ProgramError> {
        Ok(self.get_tr_address(network)?.script_pubkey())
    }

    pub fn get_script_hash(&self, network: SimplicityNetwork) -> Result<[u8; 32], ProgramError> {
        let script = self.get_script_pubkey(network)?;
        let mut hasher = Sha256::new();

        sha2::digest::Update::update(&mut hasher, script.as_bytes());
        Ok(hasher.finalize().into())
    }

    fn load(&self) -> Result<CompiledProgram, ProgramError> {
        let compiled = CompiledProgram::new(self.source, self.arguments.build_arguments(), true)
            .map_err(ProgramError::Compilation)?;
        Ok(compiled)
    }

    fn script_version(&self) -> Result<(Script, taproot::LeafVersion), ProgramError> {
        let cmr = self.load()?.commit().cmr();
        let script = script::Script::from(cmr.as_ref().to_vec());

        Ok((script, simplicityhl::simplicity::leaf_version()))
    }

    fn taproot_spending_info(&self) -> Result<taproot::TaprootSpendInfo, ProgramError> {
        let builder = taproot::TaprootBuilder::new();
        let (script, version) = self.script_version()?;

        let builder = builder
            .add_leaf_with_ver(0, script, version)
            .expect("tap tree should be valid");

        Ok(builder
            .finalize(secp256k1::SECP256K1, *self.pub_key)
            .expect("tap tree should be valid"))
    }

    fn control_block(&self) -> Result<taproot::ControlBlock, ProgramError> {
        let info = self.taproot_spending_info()?;
        let script_ver = self.script_version()?;

        Ok(info.control_block(&script_ver).expect("control block should exist"))
    }
}
