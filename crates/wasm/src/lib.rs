#![warn(clippy::all, clippy::pedantic, missing_docs)]
//! WebAssembly bindings for the Simplex SDK.
//!
//! This crate exists so the SDK itself stays free of `wasm-bindgen` annotations
//! and follows the arrangement of `lwk_wasm`.

use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;

use elements_miniscript::bitcoin::PublicKey;

use simplicityhl::ast::ElementsJetHinter;
use simplicityhl::elements::hashes::Hash;
use simplicityhl::elements::{self, Sequence};
use simplicityhl::elements::{AssetId, ContractHash, LockTime, OutPoint, Script, TxOut, Txid};
use simplicityhl::{Arguments, TemplateProgram, UnstableFeatures, WitnessValues};

use smplx_sdk::program::{ArgumentsTrait, Program, WitnessTrait};
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::partial_input::IssuanceInput;
use smplx_sdk::transaction::{
    ChangeOutput, FinalTransaction, IssuanceDetails, PartialInput, PartialOutput, ProgramInput, RequiredSignature, UTXO,
};

use wasm_bindgen::prelude::*;

/// Resolves a network name to the Simplex SDK's network enum.
fn network_from_str(network: &str) -> Result<SimplicityNetwork, JsError> {
    match network {
        "liquid" => Ok(SimplicityNetwork::Liquid),
        "liquid-testnet" | "liquidtestnet" => Ok(SimplicityNetwork::LiquidTestnet),
        "elements-regtest" | "elementsregtest" | "regtest" => Ok(SimplicityNetwork::default_regtest()),
        other => Err(JsError::new(&format!("Unknown network: {other}"))),
    }
}

/// Asset issuance details.
#[wasm_bindgen]
pub struct IssuanceReport {
    asset_id: String,
    entropy: String,
    reissuance_token_id: String,
}

#[wasm_bindgen]
impl IssuanceReport {
    fn from_details(details: &IssuanceDetails) -> Self {
        Self {
            asset_id: details.asset_id.to_string(),
            entropy: details.asset_entropy.to_string(),
            reissuance_token_id: details.inflation_asset_id.to_string(),
        }
    }

    /// The asset this issuance creates.
    #[wasm_bindgen(getter, js_name = assetId)]
    #[must_use]
    pub fn asset_id(&self) -> String {
        self.asset_id.clone()
    }

    /// The entropy to derive the reissuance asset.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn entropy(&self) -> String {
        self.entropy.clone()
    }

    /// The reissuance asset this issuance creates.
    #[wasm_bindgen(getter, js_name = reissuanceTokenId)]
    #[must_use]
    pub fn reissuance_token_id(&self) -> String {
        self.reissuance_token_id.clone()
    }
}

/// Compile-time parameters for a covenant, resolved before construction.
#[derive(Clone)]
struct FixedArguments(Arguments);

impl ArgumentsTrait for FixedArguments {
    fn build_arguments(&self) -> Arguments {
        self.0.clone()
    }
}

/// Witness values for a covenant input, resolved before the transaction is assembled.
#[derive(Clone)]
struct FixedWitness(WitnessValues);

impl WitnessTrait for FixedWitness {
    fn build_witness(&self) -> WitnessValues {
        self.0.clone()
    }
}

/// A compiled `SimplicityHL` covenant.
#[wasm_bindgen]
pub struct Covenant {
    program: Program,
}

#[wasm_bindgen]
impl Covenant {
    /// Creates a covenant from `SimplicityHL` source text delivered at runtime.
    ///
    /// `argumentsJson` carries the covenant's compile-time parameters.
    ///
    /// Shape: `{"NAME": {"value": "0x…", "type": "Pubkey"}}`.
    /// Pass `None` for a covenant that declares no parameters.
    ///
    /// `extraLeavesJson` is a JSON array of hex strings, each an encoded taproot
    /// leaf payload appended to the tree in declaration order.
    ///
    /// # Errors
    /// Returns an error if the arguments are not valid `SimplicityHL` argument JSON, or if the
    /// extra leaves are not a JSON array of hex strings.
    #[wasm_bindgen(constructor)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        source: &str,
        arguments_json: Option<String>,
        extra_leaves_json: Option<String>,
        include_debug_symbols: Option<bool>,
    ) -> Result<Covenant, JsError> {
        Ok(Self {
            program: Self::from_source(
                source,
                arguments_json.as_deref(),
                extra_leaves_json.as_deref(),
                include_debug_symbols,
            )?,
        })
    }

    /// Compiles the covenant and returns its Commitment Merkle Root as lowercase hex.
    #[wasm_bindgen(js_name = commitmentMerkleRoot)]
    #[must_use]
    pub fn commitment_merkle_root(&self) -> String {
        let cmr = self.program.get_cmr();

        hex::encode(cmr)
    }

    /// Compiles the covenant and returns the `scriptPubKey` its funds are locked with, as hex.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown or the source fails to compile.
    #[wasm_bindgen(js_name = scriptPubKeyHex)]
    pub fn script_pubkey_hex(&self, network: &str) -> Result<String, JsError> {
        let network = network_from_str(network)?;

        Ok(hex::encode(self.program.get_script_pubkey(&network).as_bytes()))
    }

    /// Compiles the covenant and returns its `scriptPubKey` hash.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown or the source fails to compile.
    #[wasm_bindgen(js_name = scriptHash)]
    pub fn script_hash(&self, network: &str) -> Result<String, JsError> {
        let network = network_from_str(network)?;

        Ok(hex::encode(self.program.get_script_hash(&network)))
    }

    /// Compiles the covenant and returns the taproot address its funds would sit at.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown or the source fails to compile.
    #[wasm_bindgen(js_name = address)]
    pub fn address(&self, network: &str) -> Result<String, JsError> {
        let network = network_from_str(network)?;

        Ok(self.program.get_tr_address(&network).to_string())
    }

    fn from_source(
        source: &str,
        arguments_json: Option<&str>,
        extra_leaves_json: Option<&str>,
        include_debug_symbols: Option<bool>,
    ) -> Result<Program, JsError> {
        let arguments = match arguments_json {
            Some(json) if !json.trim().is_empty() => serde_json::from_str::<Arguments>(json)
                .map_err(|e| JsError::new(&format!("Invalid covenant arguments: {e}")))?,
            _ => Arguments::default(),
        };

        let mut program = Program::new(Arc::<str>::from(source), &FixedArguments(arguments));

        if let Some(include) = include_debug_symbols {
            program = program.with_debug_symbols(include);
        }

        if let Some(json) = extra_leaves_json.filter(|json| !json.trim().is_empty()) {
            let leaves: Vec<String> =
                serde_json::from_str(json).map_err(|e| JsError::new(&format!("Invalid extra leaves: {e}")))?;

            program = program.with_storage_capacity(leaves.len());

            for (index, leaf) in leaves.iter().enumerate() {
                let bytes = hex::decode(leaf.strip_prefix("0x").unwrap_or(leaf))
                    .map_err(|e| JsError::new(&format!("Extra leaf {index} is not hex: {e}")))?;

                program.set_storage_at(index, bytes);
            }
        }

        Ok(program)
    }
}

/// The compile-time parameters a covenant source declares, as JSON of name to type.
///
/// # Errors
/// Returns an error if the source does not parse or does not type-check.
#[wasm_bindgen(js_name = covenantParameterTypes)]
pub fn covenant_parameter_types(source: &str) -> Result<String, JsError> {
    let template = TemplateProgram::new_with_unstable(
        Arc::<str>::from(source),
        &UnstableFeatures::all(),
        Box::new(ElementsJetHinter),
    )
    .map_err(|e| JsError::new(&format!("Covenant does not compile: {e}")))?;

    let declared: BTreeMap<String, String> = template
        .parameters()
        .iter()
        .map(|(name, ty)| (name.as_ref().to_string(), ty.to_string()))
        .collect();

    serde_json::to_string(&declared).map_err(|e| JsError::new(&format!("Cannot report parameter types: {e}")))
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
            .finalize_strict(&builder.transaction, fee_rate)
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

    /// Sets the block height this transaction may not be mined before.
    ///
    /// # Panics
    /// Panics if `height` is `500_000_000` or greater, which Elements reads as a time.
    #[wasm_bindgen(js_name = setLocktimeHeight)]
    pub fn set_locktime_height(&mut self, height: u32) {
        self.transaction.set_locktime(LockTime::from_height(height).unwrap());
    }

    /// Sets the block time this transaction may not be mined before.
    ///
    /// # Panics
    /// Panics if `time` is below `500_000_000`, which Elements reads as a height.
    #[wasm_bindgen(js_name = setLocktimeTime)]
    pub fn set_locktime_time(&mut self, time: u32) {
        self.transaction.set_locktime(LockTime::from_time(time).unwrap());
    }

    /// Sets the sequence of for this transaction.
    #[wasm_bindgen(js_name = setSequence)]
    pub fn set_sequence(&mut self, sequence: u32) {
        self.transaction.set_sequence(Sequence::from_consensus(sequence));
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
    pub fn add_wallet_input(&mut self, txid: &str, vout: u32, tx_out_hex: &str) -> Result<(), JsError> {
        self.transaction.add_input(
            PartialInput::new(Self::utxo_at(txid, vout, tx_out_hex)?),
            RequiredSignature::NativeEcdsa,
        );

        Ok(())
    }

    /// Adds an ordinary wallet input that also creates a new asset.
    ///
    /// # Errors
    /// Returns an error if the txid, the encoded output or the issuer contract cannot be parsed.
    #[wasm_bindgen(js_name = addWalletIssuanceInput)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn add_wallet_issuance_input(
        &mut self,
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        asset_amount_sats: u64,
        inflation_amount_sats: u64,
        issuer_contract_hex: Option<String>,
    ) -> Result<IssuanceReport, JsError> {
        let contract = Self::issuer_contract(issuer_contract_hex.as_deref())
            .map_err(|e| JsError::new(&format!("Invalid issuer contract: {e}")))?;

        let details = self.transaction.add_issuance_input(
            PartialInput::new(Self::utxo_at(txid, vout, tx_out_hex)?),
            IssuanceInput::new_issuance(asset_amount_sats, inflation_amount_sats, contract),
            RequiredSignature::NativeEcdsa,
        );

        Ok(IssuanceReport::from_details(&details))
    }

    /// Adds a Simplicity covenant input.
    ///
    /// `witness_json` carries the witness values in `SimplicityHL` `.wit` shape.
    /// Passing `None` leaves them unset.
    ///
    /// `signature_witness` names the witness the signer must fill with a Schnorr signature
    /// over this transaction.
    /// Leaving this `None` says the program needs no signature.
    ///
    /// # Errors
    /// Returns an error if the txid, the encoded output, the arguments or the witness cannot be parsed.
    #[wasm_bindgen(js_name = addCovenantInput)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn add_covenant_input(
        &mut self,
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        source: &str,
        arguments_json: Option<String>,
        witness_json: Option<String>,
        signature_witness: Option<String>,
        extra_leaves_json: Option<String>,
        include_debug_symbols: Option<bool>,
    ) -> Result<(), JsError> {
        self.transaction.add_program_input(
            PartialInput::new(Self::utxo_at(txid, vout, tx_out_hex)?),
            Self::program_input(
                source,
                arguments_json,
                witness_json,
                extra_leaves_json,
                include_debug_symbols,
            )?,
            Self::required_signature(signature_witness.as_deref()),
        );

        Ok(())
    }

    /// Adds a Simplicity covenant input that also creates a new asset.
    ///
    /// The covenant half is the same as `addCovenantInput` and
    /// the issuance half the same as `addWalletIssuanceInput`.
    ///
    /// # Errors
    /// Returns an error if the txid, the encoded output, the arguments, the witness or the
    /// issuer contract cannot be parsed.
    #[wasm_bindgen(js_name = addCovenantIssuanceInput)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn add_covenant_issuance_input(
        &mut self,
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        source: &str,
        arguments_json: Option<String>,
        witness_json: Option<String>,
        signature_witness: Option<String>,
        asset_amount_sats: u64,
        inflation_amount_sats: u64,
        issuer_contract_hex: Option<String>,
        extra_leaves_json: Option<String>,
        include_debug_symbols: Option<bool>,
    ) -> Result<IssuanceReport, JsError> {
        let contract = Self::issuer_contract(issuer_contract_hex.as_deref())
            .map_err(|e| JsError::new(&format!("Invalid issuer contract: {e}")))?;

        let details = self.transaction.add_program_issuance_input(
            PartialInput::new(Self::utxo_at(txid, vout, tx_out_hex)?),
            Self::program_input(
                source,
                arguments_json,
                witness_json,
                extra_leaves_json,
                include_debug_symbols,
            )?,
            IssuanceInput::new_issuance(asset_amount_sats, inflation_amount_sats, contract),
            Self::required_signature(signature_witness.as_deref()),
        );

        Ok(IssuanceReport::from_details(&details))
    }

    /// Adds an output paying `amount_sats` of `asset_hex` to `script_pubkey_hex`.
    ///
    /// A blinding key makes the output confidential. Covenant and `OP_RETURN` outputs are always unblinded.
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

    /// Runs the Simplicity program of one covenant input against this transaction.
    ///
    /// This is the dry-run: it satisfies the witness, prunes the branches the spend does not
    /// take, and executes the result on a `BitMachine`.
    ///
    /// # Errors
    /// Returns an error if the input is not a covenant input, or if the program fails to
    /// satisfy, prune or execute.
    #[wasm_bindgen(js_name = dryRunCovenantInput)]
    pub fn dry_run_covenant_input(&self, input_index: usize, network: &str) -> Result<(), JsError> {
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

    /// The issuer contract an issuance commits to, which is nothing unless one is named.
    fn issuer_contract(written: Option<&str>) -> Result<[u8; 32], String> {
        match written.map(str::trim).filter(|hex_id| !hex_id.is_empty()) {
            Some(hex_id) if hex_id.len() != 64 => Err("an id is thirty-two bytes".to_string()),
            Some(hex_id) => ContractHash::from_str(hex_id)
                .map(ContractHash::to_byte_array)
                .map_err(|e| e.to_string()),
            None => Ok([0_u8; 32]),
        }
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

    fn utxo_at(txid: &str, vout: u32, tx_out_hex: &str) -> Result<UTXO, JsError> {
        let outpoint = OutPoint {
            txid: Txid::from_str(txid).map_err(|e| JsError::new(&format!("Invalid txid: {e}")))?,
            vout,
        };

        let bytes = hex::decode(tx_out_hex).map_err(|e| JsError::new(&format!("Invalid output encoding: {e}")))?;
        let txout: TxOut =
            elements::encode::deserialize(&bytes).map_err(|e| JsError::new(&format!("Invalid output: {e}")))?;

        Ok(UTXO {
            outpoint,
            secrets: None,
            txout,
        })
    }

    fn program_input(
        source: &str,
        arguments_json: Option<String>,
        witness_json: Option<String>,
        extra_leaves_json: Option<String>,
        include_debug_symbols: Option<bool>,
    ) -> Result<ProgramInput, JsError> {
        let witness = match witness_json {
            Some(json) if !json.trim().is_empty() => serde_json::from_str::<WitnessValues>(&json)
                .map_err(|e| JsError::new(&format!("Invalid witness values: {e}")))?,
            _ => WitnessValues::default(),
        };

        Ok(ProgramInput {
            program: Box::new(Covenant::new(source, arguments_json, extra_leaves_json, include_debug_symbols)?.program),
            witness: Box::new(FixedWitness(witness)),
        })
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

#[cfg(test)]
mod tests {
    use simplicityhl::elements::hashes::sha256::Midstate;
    use simplicityhl::elements::{AssetId, OutPoint, Txid};

    use smplx_sdk::utils::asset_entropy;

    use super::{ContractHash, FromStr, Hash, IssuanceDetails, IssuanceReport, TransactionBuilder};

    const ON_CHAIN: [(&str, u32, &str, &str, &str); 4] = [
        (
            "9596d259270ef5bac0020435e6d859aea633409483ba64e232b8ba04ce288668",
            0,
            "3c7f0a53c2ff5b99590620d7f6604a7a3a7bfbaaa6aa61f7bfc7833ca03cde82",
            "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2",
            "59fe4d2127ba9f16bd6850a3e6271a166e7ed2e1669f6c107d655791c94ee98f",
        ),
        (
            "fc2535f2e4fc2ef1d19b832248e3edc2c3f4c4e3ee9c2bc51777bd738a6f9582",
            10,
            "d6cb01732239e8c317699c33ef525a8a1419ebf9a2ad318edbf8135f1665a773",
            "123465c803ae336c62180e52d94ee80d80828db54df9bedbb9860060f49de2eb",
            "2f7179e260a8046f02be25dec6abcf0a2c1bd3e6e13dd29ed67570e1e71a55b7",
        ),
        (
            "839e819d74ac98110fce63a3dab3a1075bbddcad811e0e125641989581919ab0",
            1,
            "56cbf179ec75145ef54d88ff50284175852f926bf2d8d06f3e2deedbdf623779",
            "4d4354944366ea1e33f27c37fec97504025d6062c551208f68597d1ed40ec53e",
            "bc1e0094f30bc863610baf601ede6b3dda5cdb1b7d1a7831c93f011282924da3",
        ),
        (
            "27e6bd36daef786775768a6b106053d0f2f10e03b6f278715931caa00662138d",
            3,
            "6e8198a20900717b87437261967214e2af0bb4d73c1134580b25ec597887203a",
            "beebee1a548fbb20280e539b697de076d87859a25c2983ebc55f2d8bec40abc3",
            "fc061c7585a4f166d251ef4f5afd7c63e33358582426f06070cfb286249926cb",
        ),
    ];

    fn report_for(txid: &str, vout: u32, contract: &str) -> IssuanceReport {
        let outpoint = OutPoint {
            txid: Txid::from_str(txid).expect("a chain vector's txid"),
            vout,
        };
        let entropy = asset_entropy(
            &outpoint,
            TransactionBuilder::issuer_contract(Some(contract)).expect("a chain vector's contract"),
        );

        IssuanceReport::from_details(&IssuanceDetails {
            asset_id: AssetId::from_entropy(entropy),
            inflation_asset_id: AssetId::reissuance_token_from_entropy(entropy, false),
            asset_entropy: entropy,
        })
    }

    #[test]
    fn reports_the_assets_liquid_actually_holds() {
        for (txid, vout, contract, asset, _) in ON_CHAIN {
            assert_eq!(report_for(txid, vout, contract).asset_id, asset);
        }
    }

    #[test]
    fn reports_the_reissuance_tokens_liquid_actually_holds() {
        for (txid, vout, contract, _, token) in ON_CHAIN {
            assert_eq!(report_for(txid, vout, contract).reissuance_token_id, token);
        }
    }

    #[test]
    fn reports_an_entropy_its_own_asset_can_be_rederived_from() {
        for (txid, vout, contract, asset, _) in ON_CHAIN {
            let reported = report_for(txid, vout, contract).entropy;
            let read_back = Midstate::from_str(&reported).expect("a reported entropy");

            assert_eq!(AssetId::from_entropy(read_back).to_string(), asset);
        }
    }

    #[test]
    fn commits_to_no_issuer_contract_unless_one_is_named() {
        let empty = [0_u8; 32];

        assert_eq!(TransactionBuilder::issuer_contract(None), Ok(empty));
        assert_eq!(TransactionBuilder::issuer_contract(Some("")), Ok(empty));
        assert_eq!(TransactionBuilder::issuer_contract(Some("   ")), Ok(empty));
        assert_eq!(TransactionBuilder::issuer_contract(Some(&"0".repeat(64))), Ok(empty));
    }

    #[test]
    fn id_leaves_in_the_form_it_arrived_in() {
        let written = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";
        let read = TransactionBuilder::issuer_contract(Some(written)).expect("a written id");

        assert_eq!(read[0], 0xd2);
        assert_eq!(read[31], 0xce);
        assert_eq!(ContractHash::from_byte_array(read).to_string(), written);
    }

    #[test]
    fn refuses_an_issuer_contract_that_is_not_an_id() {
        assert!(TransactionBuilder::issuer_contract(Some("not hex at all")).is_err());
        assert!(TransactionBuilder::issuer_contract(Some("00ff")).is_err());
    }
}
