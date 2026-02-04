#![warn(clippy::all, clippy::pedantic)]

//! High-level helpers for building and executing Simplicity programs on Liquid.

mod blinder;
mod constants;
mod error;
mod fee_rate_fetcher;
mod runner;
mod scripts;

#[cfg(feature = "encoding")]
pub mod encoding {
    pub use bincode::{Decode, Encode};

    use crate::error::EncodingError;

    /// Trait for binary encoding/decoding with hex string support.
    pub trait Encodable {
        /// Encode to binary bytes.
        ///
        /// # Errors
        /// Returns error if encoding fails.
        fn encode(&self) -> Result<Vec<u8>, EncodingError>
        where
            Self: Encode,
        {
            Ok(bincode::encode_to_vec(self, bincode::config::standard())?)
        }

        /// Decode from binary bytes.
        ///
        /// # Errors
        /// Returns error if decoding fails.
        fn decode(buf: &[u8]) -> Result<Self, EncodingError>
        where
            Self: Sized + Decode<()>,
        {
            Ok(bincode::decode_from_slice(buf, bincode::config::standard())?.0)
        }

        /// Encode to hex string.
        ///
        /// # Errors
        /// Returns error if encoding fails.
        fn to_hex(&self) -> Result<String, EncodingError>
        where
            Self: Encode,
        {
            Ok(hex::encode(Encodable::encode(self)?))
        }

        /// Decode from hex string.
        ///
        /// # Errors
        /// Returns error if hex decoding or binary decoding fails.
        fn from_hex(hex: &str) -> Result<Self, EncodingError>
        where
            Self: bincode::Decode<()>,
        {
            Encodable::decode(&hex::decode(hex)?)
        }
    }
}

pub use blinder::*;
pub use constants::*;
pub use error::ProgramError;

#[cfg(feature = "encoding")]
pub use error::EncodingError;

pub use runner::*;
pub use scripts::*;

pub use fee_rate_fetcher::*;

#[cfg(feature = "encoding")]
pub use encoding::Encodable;

use simplicityhl::elements::secp256k1_zkp::schnorr::Signature;

use std::collections::HashMap;
use std::sync::Arc;

use simplicityhl::num::U256;
use simplicityhl::simplicity::RedeemNode;
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;
use simplicityhl::simplicity::elements::{Address, Transaction, TxInWitness, TxOut};
use simplicityhl::simplicity::jet::Elements;
use simplicityhl::simplicity::jet::elements::{ElementsEnv, ElementsUtxo};
use simplicityhl::str::WitnessName;
use simplicityhl::tracker::TrackerLogLevel;
use simplicityhl::value::ValueConstructible;
use simplicityhl::{CompiledProgram, Value, WitnessValues, elements};

/// Embedded Simplicity source for a basic P2PK program used to sign a single input.
pub const P2PK_SOURCE: &str = include_str!("source_simf/p2pk.simf");

/// Construct a P2TR address for the embedded P2PK program and the provided public key.
///
/// # Errors
/// Returns error if the P2PK program fails to compile.
pub fn get_p2pk_address(
    x_only_public_key: &XOnlyPublicKey,
    network: SimplicityNetwork,
) -> Result<Address, ProgramError> {
    Ok(create_p2tr_address(
        get_p2pk_program(x_only_public_key)?.commit().cmr(),
        x_only_public_key,
        network.address_params(),
    ))
}

/// Compile the embedded P2PK program with the given X-only public key as argument.
///
/// # Errors
/// Returns error if program compilation fails.
pub fn get_p2pk_program(
    account_public_key: &XOnlyPublicKey,
) -> Result<CompiledProgram, ProgramError> {
    let arguments = simplicityhl::Arguments::from(HashMap::from([(
        WitnessName::from_str_unchecked("PUBLIC_KEY"),
        Value::u256(U256::from_byte_array(account_public_key.serialize())),
    )]));

    load_program(P2PK_SOURCE, arguments)
}

/// Execute the compiled P2PK program against the provided env, producing a pruned redeem node.
///
/// The `schnorr_signature` should be created by signing the `sighash_all` from the environment:
/// ```ignore
/// let sighash_all = secp256k1::Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());
/// let schnorr_signature = keypair.sign_schnorr(sighash_all);
/// ```
///
/// # Errors
/// Returns error if program execution fails.
pub fn execute_p2pk_program(
    compiled_program: &CompiledProgram,
    schnorr_signature: &Signature,
    env: &ElementsEnv<Arc<Transaction>>,
    runner_log_level: TrackerLogLevel,
) -> Result<Arc<RedeemNode<Elements>>, ProgramError> {
    let witness_values = WitnessValues::from(HashMap::from([(
        WitnessName::from_str_unchecked("SIGNATURE"),
        Value::byte_array(schnorr_signature.serialize()),
    )]));

    Ok(run_program(compiled_program, witness_values, env, runner_log_level)?.0)
}

/// Create a Schnorr signature for the P2PK program by signing the `sighash_all` of the transaction.
///
/// This is a convenience function that builds the environment and signs the transaction hash.
///
/// # Errors
/// Returns error if program compilation or environment verification fails.
pub fn create_p2pk_signature(
    tx: &Transaction,
    utxos: &[TxOut],
    keypair: &elements::schnorr::Keypair,
    input_index: usize,
    network: SimplicityNetwork,
) -> Result<Signature, ProgramError> {
    use simplicityhl::simplicity::hashes::Hash as _;

    let x_only_public_key = keypair.x_only_public_key().0;
    let p2pk_program = get_p2pk_program(&x_only_public_key)?;

    let env = get_and_verify_env(
        tx,
        &p2pk_program,
        &x_only_public_key,
        utxos,
        network,
        input_index,
    )?;

    let sighash_all =
        elements::secp256k1_zkp::Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());
    Ok(keypair.sign_schnorr(sighash_all))
}

/// Finalize the given transaction by attaching a Simplicity witness for the specified P2PK input.
///
/// The `schnorr_signature` should be created by signing the `sighash_all` from the environment.
/// Use [`create_p2pk_signature`] to create the signature if you have access to the secret key:
/// ```ignore
/// let signature = create_p2pk_signature(&tx, &utxos, &keypair, input_index, params, genesis_hash)?;
/// let tx = finalize_p2pk_transaction(tx, &utxos, &public_key, &signature, input_index, params, genesis_hash, TrackerLogLevel::None)?;
/// ```
///
/// Preconditions:
/// - `utxos[input_index]` must match the P2PK address derived from `x_only_public_key` and program CMR.
///
/// # Errors
/// Returns error if program compilation, execution, or environment verification fails.
#[allow(clippy::too_many_arguments)]
pub fn finalize_p2pk_transaction(
    mut tx: Transaction,
    utxos: &[TxOut],
    x_only_public_key: &XOnlyPublicKey,
    schnorr_signature: &Signature,
    input_index: usize,
    network: SimplicityNetwork,
    log_level: TrackerLogLevel,
) -> Result<Transaction, ProgramError> {
    let p2pk_program = get_p2pk_program(x_only_public_key)?;

    let env = get_and_verify_env(
        &tx,
        &p2pk_program,
        x_only_public_key,
        utxos,
        network,
        input_index,
    )?;

    let pruned = execute_p2pk_program(&p2pk_program, schnorr_signature, &env, log_level)?;

    let (simplicity_program_bytes, simplicity_witness_bytes) = pruned.to_vec_with_witness();
    let cmr = pruned.cmr();

    tx.input[input_index].witness = TxInWitness {
        amount_rangeproof: None,
        inflation_keys_rangeproof: None,
        script_witness: vec![
            simplicity_witness_bytes,
            simplicity_program_bytes,
            cmr.as_ref().to_vec(),
            control_block(cmr, *x_only_public_key).serialize(),
        ],
        pegin_witness: vec![],
    };

    Ok(tx)
}

/// Finalize transaction with a Simplicity witness for the specified input.
///
/// # Errors
/// Returns error if environment verification or program execution fails.
#[allow(clippy::too_many_arguments)]
pub fn finalize_transaction(
    mut tx: Transaction,
    program: &CompiledProgram,
    program_public_key: &XOnlyPublicKey,
    utxos: &[TxOut],
    input_index: usize,
    witness_values: WitnessValues,
    network: SimplicityNetwork,
    log_level: TrackerLogLevel,
) -> Result<Transaction, ProgramError> {
    let env = get_and_verify_env(
        &tx,
        program,
        program_public_key,
        utxos,
        network,
        input_index,
    )?;

    let pruned = run_program(program, witness_values, &env, log_level)?.0;

    let (simplicity_program_bytes, simplicity_witness_bytes) = pruned.to_vec_with_witness();
    let cmr = pruned.cmr();

    tx.input[input_index].witness = TxInWitness {
        amount_rangeproof: None,
        inflation_keys_rangeproof: None,
        script_witness: vec![
            simplicity_witness_bytes,
            simplicity_program_bytes,
            cmr.as_ref().to_vec(),
            control_block(cmr, *program_public_key).serialize(),
        ],
        pegin_witness: vec![],
    };

    Ok(tx)
}

/// Build and verify an Elements environment for program execution.
///
/// # Errors
/// Returns error if UTXO index is invalid or script pubkey doesn't match.
pub fn get_and_verify_env(
    tx: &Transaction,
    program: &CompiledProgram,
    program_public_key: &XOnlyPublicKey,
    utxos: &[TxOut],
    network: SimplicityNetwork,
    input_index: usize,
) -> Result<ElementsEnv<Arc<Transaction>>, ProgramError> {
    let params = network.address_params();
    let genesis_hash = network.genesis_block_hash();
    let cmr = program.commit().cmr();

    if utxos.len() <= input_index {
        return Err(ProgramError::UtxoIndexOutOfBounds {
            input_index,
            utxo_count: utxos.len(),
        });
    }

    let target_utxo = &utxos[input_index];
    let script_pubkey = create_p2tr_address(cmr, program_public_key, params).script_pubkey();

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
        control_block(cmr, *program_public_key),
        None,
        genesis_hash,
    ))
}
