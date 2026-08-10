#![warn(clippy::all, clippy::pedantic, missing_docs)]
//! WebAssembly bindings for the Simplex SDK.
//!
//! This crate exists so the SDK itself stays free of `wasm-bindgen` annotations
//! and follows the arrangement of `lwk_wasm`.

use std::str::FromStr;
use std::sync::Arc;

use elements_miniscript::bitcoin::PublicKey;

use simplicityhl::elements;
use simplicityhl::elements::{AssetId, OutPoint, Script, Sequence, TxOut, Txid};
use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::{ArgumentsTrait, Program, WitnessTrait};
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::{
    ChangeOutput, FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature, UTXO,
};

use wasm_bindgen::prelude::*;

/// Resolves a network name to the SDK's network enum.
fn network_from_str(network: &str) -> Result<SimplicityNetwork, JsError> {
    match network {
        "liquid" => Ok(SimplicityNetwork::Liquid),
        "liquid-testnet" | "liquidtestnet" => Ok(SimplicityNetwork::LiquidTestnet),
        "elements-regtest" | "elementsregtest" | "regtest" => Ok(SimplicityNetwork::default_regtest()),
        other => Err(JsError::new(&format!("Unknown network: {other}"))),
    }
}

/// Compile-time parameters for a contract, resolved before construction.
#[derive(Clone)]
struct FixedArguments(Arguments);

impl ArgumentsTrait for FixedArguments {
    fn build_arguments(&self) -> Arguments {
        self.0.clone()
    }
}

/// Witness values for a contract input, resolved before the transaction is assembled.
///
/// Held as parsed `WitnessValues` so a malformed set is rejected when the caller supplies
/// it rather than in the middle of signing.
#[derive(Clone)]
struct FixedWitness(WitnessValues);

impl WitnessTrait for FixedWitness {
    fn build_witness(&self) -> WitnessValues {
        self.0.clone()
    }
}

/// A compiled `SimplicityHL` contract.
#[wasm_bindgen]
pub struct Contract {
    program: Program,
}

#[wasm_bindgen]
impl Contract {
    /// Creates a contract from SimplicityHL source text delivered at runtime.
    ///
    /// `argumentsJson` carries the contract's compile-time parameters.
    ///
    /// Shape: `{"NAME": {"value": "0x…", "type": "Pubkey"}}`.
    /// Pass `None` for a contract that declares no parameters.
    ///
    /// `extraLeavesJson` is a JSON array of hex strings, each an encoded taproot
    /// leaf payload appended to the tree in declaration order.
    ///
    /// # Errors
    /// Returns an error if the arguments are not valid SimplicityHL argument JSON, or if the
    /// extra leaves are not a JSON array of hex strings.
    #[wasm_bindgen(constructor)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        source: &str,
        arguments_json: Option<String>,
        extra_leaves_json: Option<String>,
        include_debug_symbols: Option<bool>,
    ) -> Result<Contract, JsError> {
        let arguments = match arguments_json.as_deref() {
            Some(json) if !json.trim().is_empty() => serde_json::from_str::<Arguments>(json)
                .map_err(|e| JsError::new(&format!("Invalid contract arguments: {e}")))?,
            _ => Arguments::default(),
        };

        let mut program = Program::new(Arc::<str>::from(source), &FixedArguments(arguments));

        if let Some(include) = include_debug_symbols {
            program = program.with_debug_symbols(include);
        }

        if let Some(json) = extra_leaves_json.as_deref().filter(|json| !json.trim().is_empty()) {
            let leaves: Vec<String> =
                serde_json::from_str(json).map_err(|e| JsError::new(&format!("Invalid extra leaves: {e}")))?;

            program = program.with_storage_capacity(leaves.len());

            for (index, leaf) in leaves.iter().enumerate() {
                let bytes = hex::decode(leaf.strip_prefix("0x").unwrap_or(leaf))
                    .map_err(|e| JsError::new(&format!("Extra leaf {index} is not hex: {e}")))?;

                program.set_storage_at(index, bytes);
            }
        }

        Ok(Self { program })
    }

    /// Compiles the contract and returns its Commitment Merkle Root as lowercase hex.
    #[wasm_bindgen(js_name = commitmentMerkleRoot)]
    #[must_use]
    pub fn commitment_merkle_root(&self) -> String {
        let cmr = self.program.get_cmr();

        hex::encode(cmr)
    }

    /// Compiles the contract and returns the scriptPubKey its funds are locked with, as hex.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown or the source fails to compile.
    #[wasm_bindgen(js_name = scriptPubKeyHex)]
    pub fn script_pubkey_hex(&self, network: &str) -> Result<String, JsError> {
        let network = network_from_str(network)?;

        Ok(hex::encode(self.program.get_script_pubkey(&network).as_bytes()))
    }

    /// Compiles the contract and returns its scriptPubKey hash.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown or the source fails to compile.
    #[wasm_bindgen(js_name = scriptHash)]
    pub fn script_hash(&self, network: &str) -> Result<String, JsError> {
        let network = network_from_str(network)?;

        Ok(hex::encode(self.program.get_script_hash(&network)))
    }

    /// Compiles the contract and returns the taproot address its funds would sit at.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown or the source fails to compile.
    #[wasm_bindgen(js_name = contractAddress)]
    pub fn contract_address(&self, network: &str) -> Result<String, JsError> {
        let network = network_from_str(network)?;

        Ok(self.program.get_tr_address(&network).to_string())
    }
}

/// The wallet's signer that understands how to work with Simplicity.
#[wasm_bindgen]
pub struct WalletSigner {
    signer: Signer,
    network: SimplicityNetwork,
}

#[wasm_bindgen]
impl WalletSigner {
    /// Creates a signer from an account mnemonic.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown.
    #[wasm_bindgen(constructor)]
    pub fn new(mnemonic: &str, network: &str) -> Result<WalletSigner, JsError> {
        let network = network_from_str(network)?;

        Ok(Self {
            signer: Signer::from_mnemonic(mnemonic, network),
            network,
        })
    }

    /// The WPKH address of the signer's own key.
    #[wasm_bindgen(js_name = address)]
    #[must_use]
    pub fn address(&self) -> String {
        let _ = &self.network;

        self.signer.get_address().to_string()
    }

    /// The confidential WPKH address of the signer's own key.
    #[wasm_bindgen(js_name = confidentialAddress)]
    #[must_use]
    pub fn confidential_address(&self) -> String {
        self.signer.get_confidential_address().to_string()
    }

    /// The x-only public key used for Schnorr and taproot, as lowercase hex.
    #[wasm_bindgen(js_name = schnorrPublicKey)]
    #[must_use]
    pub fn schnorr_public_key(&self) -> String {
        hex::encode(self.signer.get_schnorr_public_key().serialize())
    }

    /// The compressed public key used for ordinary wallet inputs, as lowercase hex.
    #[wasm_bindgen(js_name = ecdsaPublicKey)]
    #[must_use]
    pub fn ecdsa_public_key(&self) -> String {
        hex::encode(self.signer.get_ecdsa_public_key().to_bytes())
    }

    /// The scriptPubKey of the signer's own address, as lowercase hex.
    #[wasm_bindgen(js_name = scriptPubKeyHex)]
    #[must_use]
    pub fn script_pubkey_hex(&self) -> String {
        hex::encode(self.signer.get_address().script_pubkey().as_bytes())
    }

    /// The blinding public key, as lowercase hex.
    #[wasm_bindgen(js_name = blindingPublicKey)]
    #[must_use]
    pub fn blinding_public_key(&self) -> String {
        hex::encode(self.signer.get_blinding_public_key().to_bytes())
    }

    /// Blinds, signs and finalizes an assembled transaction.
    ///
    /// # Errors
    /// Returns an error if the transaction cannot be balanced, blinded, signed or finalized.
    #[wasm_bindgen(js_name = finalizeTransaction)]
    pub fn finalize_transaction(
        &self,
        builder: &TransactionBuilder,
        fee_rate: f32,
    ) -> Result<SignedTransaction, JsError> {
        let (transaction, fee_sats) = self
            .signer
            .finalize_strict(builder.inner(), fee_rate)
            .map_err(|e| JsError::new(&format!("Could not finalize the transaction: {e}")))?;

        Ok(SignedTransaction {
            fee_sats,
            hex: elements::encode::serialize_hex(&transaction),
            txid: transaction.txid().to_string(),
        })
    }
}

/// A transaction under construction.
///
/// Inputs are expected as an outpoint plus the raw `TxOut` they spend.
/// Coin selection and unblinding are the caller's responsibility.
///
/// Assembles exactly what it is given and adds only the change and fee outputs.
#[wasm_bindgen]
pub struct TransactionBuilder {
    transaction: FinalTransaction,
}

#[wasm_bindgen]
impl TransactionBuilder {
    /// Starts an empty transaction.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            transaction: FinalTransaction::new(),
        }
    }

    /// Sets where this transaction's change should go.
    ///
    /// Left unset, change returns to the signer's own derived address.
    ///
    /// # Errors
    /// Returns an error if the script or the blinding key cannot be parsed.
    #[wasm_bindgen(js_name = addChange)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_change(&mut self, script_pubkey_hex: &str, blinding_key_hex: Option<String>) -> Result<(), JsError> {
        let script = Script::from(
            hex::decode(script_pubkey_hex).map_err(|e| JsError::new(&format!("Invalid change script: {e}")))?,
        );

        let mut change = ChangeOutput::new(script);

        if let Some(blinding_key) = blinding_key_hex.as_deref() {
            let key = PublicKey::from_str(blinding_key)
                .map_err(|e| JsError::new(&format!("Invalid change blinding key: {e}")))?;

            change = change.with_blinding_key(key);
        }

        self.transaction.add_change(change);

        Ok(())
    }

    /// Drops the change target, returning to the signer's own address.
    #[wasm_bindgen(js_name = removeChange)]
    pub fn remove_change(&mut self) {
        self.transaction.remove_change();
    }

    /// Adds an ordinary wallet input, spending the output at `txid:vout`.
    ///
    /// `tx_out_hex` is the consensus encoding of the output being spent, which is what the
    /// wallet already has from its own snapshot or a chain read.
    ///
    /// # Errors
    /// Returns an error if the txid or the encoded output cannot be parsed.
    #[wasm_bindgen(js_name = addWalletInput)]
    pub fn add_wallet_input(
        &mut self,
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        sequence: Option<u32>,
    ) -> Result<(), JsError> {
        let outpoint = OutPoint {
            txid: Txid::from_str(txid).map_err(|e| JsError::new(&format!("Invalid txid: {e}")))?,
            vout,
        };

        let bytes = hex::decode(tx_out_hex).map_err(|e| JsError::new(&format!("Invalid output encoding: {e}")))?;
        let txout: TxOut =
            elements::encode::deserialize(&bytes).map_err(|e| JsError::new(&format!("Invalid output: {e}")))?;

        self.transaction.add_input(
            Self::with_sequence(
                PartialInput::new(UTXO {
                    outpoint,
                    secrets: None,
                    txout,
                }),
                sequence,
            ),
            RequiredSignature::NativeEcdsa,
        );

        Ok(())
    }

    /// Adds a Simplicity contract input, spent by satisfying it.
    ///
    /// `witness_json` carries the witness values in SimplicityHL's `.wit` shape.
    /// Passing `None` leaves them unset.
    ///
    /// `signature_witness` names the witness the signer must fill with a Schnorr signature
    /// over this transaction.
    /// Leaving this `None` says the program needs no signature.
    ///
    /// # Errors
    /// Returns an error if the txid, the encoded output, the arguments or the witness cannot be parsed.
    #[wasm_bindgen(js_name = addContractInput)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn add_contract_input(
        &mut self,
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        source: &str,
        arguments_json: Option<String>,
        witness_json: Option<String>,
        signature_witness: Option<String>,
        sequence: Option<u32>,
    ) -> Result<(), JsError> {
        let outpoint = OutPoint {
            txid: Txid::from_str(txid).map_err(|e| JsError::new(&format!("Invalid txid: {e}")))?,
            vout,
        };

        let bytes = hex::decode(tx_out_hex).map_err(|e| JsError::new(&format!("Invalid output encoding: {e}")))?;
        let txout: TxOut =
            elements::encode::deserialize(&bytes).map_err(|e| JsError::new(&format!("Invalid output: {e}")))?;

        let arguments = match arguments_json.as_deref() {
            Some(json) if !json.trim().is_empty() => serde_json::from_str::<Arguments>(json)
                .map_err(|e| JsError::new(&format!("Invalid contract arguments: {e}")))?,
            _ => Arguments::default(),
        };

        let witness = match witness_json.as_deref() {
            Some(json) if !json.trim().is_empty() => serde_json::from_str::<WitnessValues>(json)
                .map_err(|e| JsError::new(&format!("Invalid witness values: {e}")))?,
            _ => WitnessValues::default(),
        };

        let program = Program::new(Arc::<str>::from(source), &FixedArguments(arguments));

        self.transaction.add_program_input(
            Self::with_sequence(
                PartialInput::new(UTXO {
                    outpoint,
                    secrets: None,
                    txout,
                }),
                sequence,
            ),
            ProgramInput {
                program: Box::new(program),
                witness: Box::new(FixedWitness(witness)),
            },
            Self::required_signature(signature_witness.as_deref()),
        );

        Ok(())
    }

    /// Adds an output paying `amount_sats` of `asset_hex` to `script_pubkey_hex`.
    ///
    /// A blinding key makes the output confidential. Covenant and OP_RETURN outputs are always unblinded.
    ///
    /// # Errors
    /// Returns an error if the script, asset id or blinding key cannot be parsed.
    #[wasm_bindgen(js_name = addOutput)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_output(
        &mut self,
        script_pubkey_hex: &str,
        amount_sats: u64,
        asset_hex: &str,
        blinding_key_hex: Option<String>,
    ) -> Result<(), JsError> {
        let script =
            Script::from(hex::decode(script_pubkey_hex).map_err(|e| JsError::new(&format!("Invalid script: {e}")))?);
        let asset = AssetId::from_str(asset_hex).map_err(|e| JsError::new(&format!("Invalid asset id: {e}")))?;

        let mut output = PartialOutput::new(script, amount_sats, asset);

        if let Some(blinding_key) = blinding_key_hex.as_deref() {
            let key =
                PublicKey::from_str(blinding_key).map_err(|e| JsError::new(&format!("Invalid blinding key: {e}")))?;

            output = output.with_blinding_key(key);
        }

        self.transaction.add_output(output);

        Ok(())
    }

    /// Runs the Simplicity program of one contract input against this transaction.
    ///
    /// This is the dry-run: it satisfies the witness, prunes the branches the spend does not
    /// take, and executes the result on a BitMachine.
    ///
    /// # Errors
    /// Returns an error if the input is not a contract input, or if the program fails to
    /// satisfy, prune or execute.
    #[wasm_bindgen(js_name = dryRunContractInput)]
    pub fn dry_run_contract_input(&self, input_index: usize, network: &str) -> Result<(), JsError> {
        let network = network_from_str(network)?;
        let inputs = self.transaction.inputs();
        let input = inputs
            .get(input_index)
            .ok_or_else(|| JsError::new(&format!("There is no input at index {input_index}.")))?;
        let program_input = input
            .program_input
            .as_ref()
            .ok_or_else(|| JsError::new(&format!("Input {input_index} is not a covenant input.")))?;

        let (pst, _secrets) = self.transaction.extract_pst();

        program_input
            .program
            .execute(&pst, &program_input.witness.build_witness(), input_index, &network)
            .map_err(|e| JsError::new(&format!("Input {input_index} did not execute: {e}")))?;

        Ok(())
    }

    /// How many inputs and outputs this transaction currently carries.
    #[wasm_bindgen(js_name = inputCount)]
    #[must_use]
    pub fn input_count(&self) -> usize {
        self.transaction.n_inputs()
    }

    /// How many outputs this transaction currently carries.
    #[wasm_bindgen(js_name = outputCount)]
    #[must_use]
    pub fn output_count(&self) -> usize {
        self.transaction.n_outputs()
    }

    /// Which signature a covenant input needs. Can either be a witness name like `SIGNATURE`,
    /// or a withess path if the signature is embedded like `SIGNATURE.Left.Right.1`
    ///
    /// `None` means no signature is required.
    fn required_signature(signature_witness: Option<&str>) -> RequiredSignature {
        let Some(raw) = signature_witness.map(str::trim).filter(|name| !name.is_empty()) else {
            return RequiredSignature::None;
        };

        let mut segments = raw.split('.').map(str::trim).filter(|part| !part.is_empty());

        let Some(name) = segments.next() else {
            return RequiredSignature::None;
        };

        let path: Vec<&str> = segments.collect();

        if path.is_empty() {
            return RequiredSignature::Witness(name.to_string());
        }

        RequiredSignature::witness_with_path(name, path)
    }

    /// Applies a declared sequence to an input.
    fn with_sequence(input: PartialInput, sequence: Option<u32>) -> PartialInput {
        match sequence {
            Some(value) => input.with_sequence(Sequence(value)),
            None => input,
        }
    }

    /// The assembled transaction, for the signer in this crate.
    fn inner(&self) -> &FinalTransaction {
        &self.transaction
    }
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A finished transaction and the fee it pays.
#[wasm_bindgen]
pub struct SignedTransaction {
    fee_sats: u64,
    hex: String,
    txid: String,
}

#[wasm_bindgen]
impl SignedTransaction {
    /// The consensus-encoded transaction, ready to broadcast.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn hex(&self) -> String {
        self.hex.clone()
    }

    /// The transaction id it will have once broadcast.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn txid(&self) -> String {
        self.txid.clone()
    }

    /// The fee it pays, in satoshis.
    #[wasm_bindgen(getter, js_name = feeSats)]
    #[must_use]
    pub fn fee_sats(&self) -> u64 {
        self.fee_sats
    }
}

/// The version of the Simplex SDK compiled into this module.
#[wasm_bindgen(js_name = sdkVersion)]
#[must_use]
pub fn sdk_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
