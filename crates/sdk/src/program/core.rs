use std::sync::Arc;

use dyn_clone::DynClone;

use simplicityhl::CompiledProgram;
use simplicityhl::WitnessValues;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::{Address, Script, Transaction, TxOut, script, taproot};
use simplicityhl::simplicity::bitcoin::{XOnlyPublicKey, secp256k1};
use simplicityhl::simplicity::jet::Elements;
use simplicityhl::simplicity::jet::elements::{ElementsEnv, ElementsUtxo};
use simplicityhl::simplicity::{BitMachine, RedeemNode, Value};
use simplicityhl::tracker::{DefaultTracker, TrackerLogLevel};

use super::arguments::ArgumentsTrait;
use super::error::ProgramError;

use crate::provider::SimplicityNetwork;
use crate::utils::hash_script;

pub trait ProgramTrait: DynClone {
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
}

dyn_clone::clone_trait_object!(ProgramTrait);

impl ProgramTrait for Program {
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
        let script_pubkey = self.get_tr_address(network)?.script_pubkey();

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
    pub fn new(source: &'static str, pub_key: XOnlyPublicKey, arguments: Box<dyn ArgumentsTrait>) -> Self {
        Self {
            source,
            pub_key,
            arguments,
        }
    }

    pub fn get_tr_address(&self, network: &SimplicityNetwork) -> Result<Address, ProgramError> {
        let spend_info = self.taproot_spending_info()?;

        Ok(Address::p2tr(
            secp256k1::SECP256K1,
            spend_info.internal_key(),
            spend_info.merkle_root(),
            None,
            network.address_params(),
        ))
    }

    pub fn get_script_pubkey(&self, network: &SimplicityNetwork) -> Result<Script, ProgramError> {
        Ok(self.get_tr_address(network)?.script_pubkey())
    }

    pub fn get_script_hash(&self, network: &SimplicityNetwork) -> Result<[u8; 32], ProgramError> {
        Ok(hash_script(&self.get_script_pubkey(network)?))
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
        elements::{AssetId, TxOutWitness, confidential, pset::Input},
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

    fn dummy_pubkey(seed: u64) -> XOnlyPublicKey {
        let mut rng = <secp256k1::rand::rngs::StdRng as secp256k1::rand::SeedableRng>::seed_from_u64(seed);
        secp256k1::Keypair::new_global(&mut rng).x_only_public_key().0
    }

    fn dummy_program() -> Program {
        Program::new(DUMMY_PROGRAM, dummy_pubkey(0), Box::new(EmptyArguments))
    }

    fn dummy_network() -> SimplicityNetwork {
        SimplicityNetwork::default_regtest()
    }

    fn make_pst_with_script(script: Script) -> PartiallySignedTransaction {
        let txout = TxOut {
            asset: confidential::Asset::Explicit(dummy_asset_id(0xAA)),
            value: confidential::Value::Explicit(1000),
            nonce: confidential::Nonce::Null,
            script_pubkey: script,
            witness: TxOutWitness::default(),
        };

        let input = Input {
            witness_utxo: Some(txout),
            ..Default::default()
        };

        let mut pst = PartiallySignedTransaction::new_v2();
        pst.add_input(input);

        pst
    }

    fn dummy_asset_id(byte: u8) -> AssetId {
        AssetId::from_slice(&[byte; 32]).unwrap()
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
            nonce: confidential::Nonce::Null,
            script_pubkey: correct_script,
            witness: TxOutWitness::default(),
        };
        pst.add_input(Input {
            witness_utxo: Some(correct_txout),
            ..Default::default()
        });

        // take script with wrong pubkey
        assert!(matches!(
            program.get_env(&pst, 0, &network).unwrap_err(),
            ProgramError::ScriptPubkeyMismatch { .. }
        ));

        assert!(program.get_env(&pst, 1, &network).is_ok());
    }
}
