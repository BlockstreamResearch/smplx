use std::collections::HashMap;
use std::str::FromStr;

use elements_miniscript::bitcoin::PublicKey;
use elements_miniscript::bitcoin::bip32::DerivationPath;

use simplicityhl::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use simplicityhl::elements::{
    self, AssetEntropy, AssetId, ContractHash, LockTime, OutPoint, Script, Sequence, TxOut, TxOutSecrets, Txid,
};
use simplicityhl::types::StructuralType;
use simplicityhl::value::StructuralValue;
use simplicityhl::{ResolvedType, TemplateProgramWitness, Value, WitnessNameToValueMap, WitnessValues};

use smplx_sdk::signer::Signer as SdkSigner;
use smplx_sdk::transaction::partial_input::IssuanceInput;
use smplx_sdk::transaction::{
    ChangeOutput, FinalTransaction, NativeSignature, PartialInput, PartialOutput, ProgramInput, ProgramSignature, UTXO,
};

use wasm_bindgen::prelude::*;

use crate::issuance_details::IssuanceDetails;
use crate::program::{FixedWitness, Program};

/// A transaction under construction.
///
/// Inputs are expected as an outpoint plus the raw `TxOut` they spend.
/// Coin selection and unblinding are the caller's responsibility.
#[wasm_bindgen]
pub struct TransactionBuilder {
    pub(crate) transaction: FinalTransaction,
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
    /// Unless a sequence is set, the inputs get one that enforces the locktime.
    ///
    /// # Panics
    /// Panics if `height` is `500_000_000` or greater, which Elements reads as a time.
    #[wasm_bindgen(js_name = setLocktimeHeight)]
    pub fn set_locktime_height(&mut self, height: u32) {
        self.transaction.set_locktime(LockTime::from_height(height).unwrap());
    }

    /// Sets the block time this transaction may not be mined before.
    /// Unless a sequence is set, the inputs get one that enforces the locktime.
    ///
    /// # Panics
    /// Panics if `time` is below `500_000_000`, which Elements reads as a height.
    #[wasm_bindgen(js_name = setLocktimeTime)]
    pub fn set_locktime_time(&mut self, time: u32) {
        self.transaction.set_locktime(LockTime::from_time(time).unwrap());
    }

    /// Sets the sequence of the inputs that don't declare their own.
    #[wasm_bindgen(js_name = setSequence)]
    pub fn set_sequence(&mut self, sequence: u32) {
        self.transaction.set_sequence(Sequence::from_consensus(sequence));
    }

    /// Sets the sequence of input `input_index` alone, such as the relative timelock a program
    /// checks, overriding `setSequence`.
    ///
    /// # Errors
    /// Returns an error if there is no input at `input_index`.
    #[wasm_bindgen(js_name = setInputSequence)]
    pub fn set_input_sequence(&mut self, input_index: usize, sequence: u32) -> Result<(), JsError> {
        self.partial_input_mut(input_index)?.sequence = Some(Sequence::from_consensus(sequence));

        Ok(())
    }

    /// Sets the block height input `input_index` alone may not be mined before, overriding
    /// `setLocktimeHeight`. Unless a sequence is set, the inputs get one that enforces the
    /// locktime.
    ///
    /// # Errors
    /// Returns an error if there is no input at `input_index`, or if `height` is `500_000_000` or
    /// greater, which Elements reads as a time.
    #[wasm_bindgen(js_name = setInputLocktimeHeight)]
    pub fn set_input_locktime_height(&mut self, input_index: usize, height: u32) -> Result<(), JsError> {
        let locktime = LockTime::from_height(height).map_err(|e| JsError::new(&format!("Invalid height: {e}")))?;

        self.partial_input_mut(input_index)?.locktime = Some(locktime);

        Ok(())
    }

    /// Sets the block time input `input_index` alone may not be mined before, overriding
    /// `setLocktimeTime`. Unless a sequence is set, the inputs get one that enforces the
    /// locktime.
    ///
    /// # Errors
    /// Returns an error if there is no input at `input_index`, or if `time` is below
    /// `500_000_000`, which Elements reads as a height.
    #[wasm_bindgen(js_name = setInputLocktimeTime)]
    pub fn set_input_locktime_time(&mut self, input_index: usize, time: u32) -> Result<(), JsError> {
        let locktime = LockTime::from_time(time).map_err(|e| JsError::new(&format!("Invalid time: {e}")))?;

        self.partial_input_mut(input_index)?.locktime = Some(locktime);

        Ok(())
    }

    /// Gives each witness in `names` of program input `input_index` the zero value of its type.
    ///
    /// For witnesses on branches the spend doesn't take: satisfying the program needs a value for
    /// every witness it declares, and pruning drops the unused ones.
    ///
    /// # Errors
    /// Returns an error if input `input_index` isn't a program input, or if its program doesn't
    /// compile or declares no witness with one of the names.
    #[wasm_bindgen(js_name = setUnusedWitnesses)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn set_unused_witnesses(&mut self, input_index: usize, names: Vec<String>) -> Result<(), JsError> {
        let program_input = self
            .transaction
            .inputs_mut()
            .get_mut(input_index)
            .and_then(|input| input.program_input.as_mut())
            .ok_or_else(|| JsError::new(&format!("Input {input_index} is not a program input.")))?;

        let types = program_input
            .program
            .get_witness_types()
            .map_err(|e| JsError::new(&format!("Program does not compile: {e}")))?;
        let mut values: HashMap<TemplateProgramWitness, Value> = program_input
            .witness
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();

        for name in &names {
            let key = TemplateProgramWitness::witness_from_str(name.as_str());
            let ty = types
                .get(&key)
                .ok_or_else(|| JsError::new(&format!("The program declares no witness named {name}.")))?;

            values.insert(key, Self::zero_value(ty));
        }

        program_input.witness = WitnessValues::from_map(values);

        Ok(())
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

    /// Adds a native input, spending the output at `txid:vout`.
    ///
    /// `tx_out_hex` is the consensus encoding of the output being spent, which is what the
    /// wallet already has from its own snapshot or a chain read.
    ///
    /// `blinding_secrets_json` contains the unblinded details of a confidential output.
    /// Write its `value` as a JSON number up to 2^53 - 1. Larger amounts must be
    /// decimal strings because JavaScript may serialize a number with different digits.
    /// The blinding factors are hex with their bytes reversed, the order Elements prints them in,
    /// so factor 1 is `01` followed by 31 zero bytes.
    ///
    /// # Errors
    /// Returns an error if the txid or the encoded output cannot be parsed.
    #[wasm_bindgen(js_name = addInput)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_input(
        &mut self,
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        blinding_secrets_json: Option<String>,
        derivation_path: Option<String>,
    ) -> Result<(), JsError> {
        let input = Self::partial_input(
            txid,
            vout,
            tx_out_hex,
            blinding_secrets_json.as_deref(),
            derivation_path.as_deref(),
        )?;

        self.transaction.add_input(input, Some(NativeSignature::Ecdsa));

        Ok(())
    }

    /// Adds a native input that also creates a new asset.
    ///
    /// # Errors
    /// Returns an error if the txid, the encoded output or the issuer contract cannot be parsed,
    /// if either issuance amount exceeds `i64::MAX`, or if both are zero.
    #[wasm_bindgen(js_name = addIssuanceInput)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn add_issuance_input(
        &mut self,
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        asset_amount_sats: u64,
        inflation_amount_sats: u64,
        issuer_contract_hex: Option<String>,
        blinding_secrets_json: Option<String>,
        derivation_path: Option<String>,
    ) -> Result<IssuanceDetails, JsError> {
        let input = Self::partial_input(
            txid,
            vout,
            tx_out_hex,
            blinding_secrets_json.as_deref(),
            derivation_path.as_deref(),
        )?;
        let issuance = Self::issuance(asset_amount_sats, inflation_amount_sats, issuer_contract_hex.as_deref())?;

        let details = self
            .transaction
            .add_issuance_input(input, issuance, Some(NativeSignature::Ecdsa));

        Ok(IssuanceDetails::from_details(&details))
    }

    /// Adds a native input that spends an output holding the reissuance token and mints
    /// `asset_amount_sats` more units of the asset.
    ///
    /// Pass the entropy returned by `IssuanceDetails.assetEntropy` for the first issuance. Like
    /// `assetEntropy`, it is hex with its bytes reversed, the order Elements prints it in.
    /// Pass the unblinded token details as `blinding_secrets_json`. The method checks
    /// that the claimed asset ID matches the token derived from the entropy. Write `value` as a
    /// decimal string if it exceeds 2^53 - 1.
    ///
    /// The token's asset must be confidential. Elements uses its asset blinding
    /// factor as the reissuance nonce. An explicit asset has a zero nonce, which
    /// creates a different asset.
    ///
    /// This method supports tokens from explicit-amount issuances, including
    /// those created by `addIssuanceInput`.
    ///
    /// # Errors
    /// Returns an error for invalid input data, an amount outside 1..=`i64::MAX`,
    /// a claimed token asset that does not match the entropy, or a token with a zero asset blinding factor.
    #[wasm_bindgen(js_name = addReissuanceInput)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn add_reissuance_input(
        &mut self,
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        asset_amount_sats: u64,
        asset_entropy: &str,
        blinding_secrets_json: &str,
        derivation_path: Option<String>,
    ) -> Result<IssuanceDetails, JsError> {
        let input = Self::partial_input(
            txid,
            vout,
            tx_out_hex,
            Some(blinding_secrets_json),
            derivation_path.as_deref(),
        )?;

        let reissuance = Self::reissuance(&input, asset_amount_sats, asset_entropy)?;

        let details = self
            .transaction
            .add_issuance_input(input, reissuance, Some(NativeSignature::Ecdsa));

        Ok(IssuanceDetails::from_details(&details))
    }

    /// Adds an input that spends an output locked by a Simplicity program.
    ///
    /// `witness_json` carries the witness values in `SimplicityHL` `.wit` shape.
    /// Passing `None` leaves them unset.
    ///
    /// `signature_witness` names the witness the signer must fill with a Schnorr signature
    /// over this transaction.
    /// Leaving this `None` says the program needs no signature.
    ///
    /// `blinding_secrets_json` contains the unblinded details of a confidential output, as in
    /// `addInput`.
    ///
    /// `derivation_path` is the path of the key that signature is made with, relative to the
    /// account path. Omitting it keeps the signer's default.
    ///
    /// # Errors
    /// Returns an error if the txid, the encoded output, the arguments, the witness, the
    /// blinding secrets or the derivation path cannot be parsed.
    #[wasm_bindgen(js_name = addProgramInput)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn add_program_input(
        &mut self,
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        source: &str,
        arguments_json: Option<String>,
        witness_json: Option<String>,
        program_signature: Option<String>,
        blinding_secrets_json: Option<String>,
        extra_leaves_json: Option<String>,
        include_debug_symbols: Option<bool>,
        derivation_path: Option<String>,
    ) -> Result<(), JsError> {
        self.transaction.add_program_input(
            Self::partial_input(
                txid,
                vout,
                tx_out_hex,
                blinding_secrets_json.as_deref(),
                derivation_path.as_deref(),
            )?,
            Self::program_input(
                source,
                arguments_json,
                witness_json,
                extra_leaves_json,
                include_debug_symbols,
            )?,
            Self::program_signature(program_signature.as_deref()),
        );

        Ok(())
    }

    /// Adds a program input, as `addProgramInput` does, that also creates a new asset.
    ///
    /// The program half is the same as `addProgramInput` and
    /// the issuance half the same as `addIssuanceInput`.
    ///
    /// # Errors
    /// Returns an error if the txid, the encoded output, the arguments, the witness, the
    /// issuer contract, the blinding secrets or the derivation path cannot be parsed, if either
    /// issuance amount exceeds `i64::MAX`, or if both are zero.
    #[wasm_bindgen(js_name = addProgramIssuanceInput)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn add_program_issuance_input(
        &mut self,
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        source: &str,
        arguments_json: Option<String>,
        witness_json: Option<String>,
        program_signature: Option<String>,
        asset_amount_sats: u64,
        inflation_amount_sats: u64,
        issuer_contract_hex: Option<String>,
        blinding_secrets_json: Option<String>,
        extra_leaves_json: Option<String>,
        include_debug_symbols: Option<bool>,
        derivation_path: Option<String>,
    ) -> Result<IssuanceDetails, JsError> {
        let input = Self::partial_input(
            txid,
            vout,
            tx_out_hex,
            blinding_secrets_json.as_deref(),
            derivation_path.as_deref(),
        )?;
        let issuance = Self::issuance(asset_amount_sats, inflation_amount_sats, issuer_contract_hex.as_deref())?;

        let details = self.transaction.add_program_issuance_input(
            input,
            Self::program_input(
                source,
                arguments_json,
                witness_json,
                extra_leaves_json,
                include_debug_symbols,
            )?,
            issuance,
            Self::program_signature(program_signature.as_deref()),
        );

        Ok(IssuanceDetails::from_details(&details))
    }

    /// Adds a program input, as `addProgramInput` does, that spends an output holding the
    /// reissuance token and mints `asset_amount_sats` more units of the asset.
    ///
    /// The program half is the same as `addProgramInput` and the reissuance half the same as
    /// `addReissuanceInput`, including the unblinded token details in `blinding_secrets_json`.
    ///
    /// # Errors
    /// Returns an error for invalid input data, an amount outside 1..=`i64::MAX`, a claimed token
    /// asset that does not match the entropy, or a token with a zero asset blinding factor.
    #[wasm_bindgen(js_name = addProgramReissuanceInput)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn add_program_reissuance_input(
        &mut self,
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        source: &str,
        arguments_json: Option<String>,
        witness_json: Option<String>,
        program_signature: Option<String>,
        asset_amount_sats: u64,
        asset_entropy: &str,
        blinding_secrets_json: &str,
        extra_leaves_json: Option<String>,
        include_debug_symbols: Option<bool>,
        derivation_path: Option<String>,
    ) -> Result<IssuanceDetails, JsError> {
        let input = Self::partial_input(
            txid,
            vout,
            tx_out_hex,
            Some(blinding_secrets_json),
            derivation_path.as_deref(),
        )?;
        let reissuance = Self::reissuance(&input, asset_amount_sats, asset_entropy)?;

        let details = self.transaction.add_program_issuance_input(
            input,
            Self::program_input(
                source,
                arguments_json,
                witness_json,
                extra_leaves_json,
                include_debug_symbols,
            )?,
            reissuance,
            Self::program_signature(program_signature.as_deref()),
        );

        Ok(IssuanceDetails::from_details(&details))
    }

    /// Removes input `input_index`, with everything set on that input. The inputs after the
    /// removed one move down by one index.
    ///
    /// # Errors
    /// Returns an error if there is no input at `input_index`.
    #[wasm_bindgen(js_name = removeInput)]
    pub fn remove_input(&mut self, input_index: usize) -> Result<(), JsError> {
        self.transaction
            .remove_input(input_index)
            .map(|_| ())
            .ok_or_else(|| JsError::new(&format!("There is no input at index {input_index}.")))
    }

    /// Adds an output paying `amount_sats` of `asset_hex` to `script_pubkey_hex`.
    ///
    /// A blinding key makes the output confidential, whatever its script.
    ///
    /// # Errors
    /// Returns an error if the script, asset id or blinding key cannot be parsed,
    /// or if the amount exceeds `i64::MAX`.
    #[wasm_bindgen(js_name = addOutput)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_output(
        &mut self,
        script_pubkey_hex: &str,
        amount_sats: u64,
        asset_hex: &str,
        blinding_key_hex: Option<String>,
    ) -> Result<(), JsError> {
        if i64::try_from(amount_sats).is_err() {
            return Err(JsError::new("output amount must not exceed i64::MAX"));
        }

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

    /// Removes output `output_index`. The outputs after the removed one move down by one index.
    ///
    /// # Errors
    /// Returns an error if there is no output at `output_index`.
    #[wasm_bindgen(js_name = removeOutput)]
    pub fn remove_output(&mut self, output_index: usize) -> Result<(), JsError> {
        self.transaction
            .remove_output(output_index)
            .map(|_| ())
            .ok_or_else(|| JsError::new(&format!("There is no output at index {output_index}.")))
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

    fn partial_input_mut(&mut self, input_index: usize) -> Result<&mut PartialInput, JsError> {
        self.transaction
            .inputs_mut()
            .get_mut(input_index)
            .map(|input| &mut input.partial_input)
            .ok_or_else(|| JsError::new(&format!("There is no input at index {input_index}.")))
    }

    fn partial_input(
        txid: &str,
        vout: u32,
        tx_out_hex: &str,
        secrets_json: Option<&str>,
        derivation_path: Option<&str>,
    ) -> Result<PartialInput, JsError> {
        let input = PartialInput::new(Self::utxo(txid, vout, tx_out_hex, secrets_json)?);

        let Some(path) = derivation_path else {
            return Ok(input);
        };

        let path = Self::relative_path(path).map_err(|e| JsError::new(&format!("Invalid derivation path: {e}")))?;

        Ok(input.with_derivation_path(path))
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
            program: Box::new(Program::new(source, arguments_json, extra_leaves_json, include_debug_symbols)?.program),
            witness: FixedWitness(witness).into(),
        })
    }

    /// Which signature a program input needs: a witness name like `SIGNATURE`, or a witness path
    /// like `SIGNATURE.Left.Right.1` when the signature sits inside the field.
    ///
    /// `None` means no signature is required.
    fn program_signature(program_signature: Option<&str>) -> Option<ProgramSignature> {
        let mut segments = program_signature?
            .split('.')
            .map(str::trim)
            .filter(|part| !part.is_empty());

        Some(ProgramSignature::new(segments.next()?).at_path(segments))
    }

    /// An issuance of `asset_amount_sats` of a new asset and `inflation_amount_sats` of its
    /// reissuance token, committing to `issuer_contract_hex`.
    fn issuance(
        asset_amount_sats: u64,
        inflation_amount_sats: u64,
        issuer_contract_hex: Option<&str>,
    ) -> Result<IssuanceInput, JsError> {
        if i64::try_from(asset_amount_sats).is_err() {
            return Err(JsError::new("asset issuance amount must not exceed i64::MAX"));
        }
        if i64::try_from(inflation_amount_sats).is_err() {
            return Err(JsError::new("inflation amount must not exceed i64::MAX"));
        }
        if asset_amount_sats == 0 && inflation_amount_sats == 0 {
            return Err(JsError::new(
                "an issuance must create the asset, its reissuance token, or both",
            ));
        }

        let contract = Self::issuer_contract(issuer_contract_hex)
            .map_err(|e| JsError::new(&format!("Invalid issuer contract: {e}")))?;

        Ok(IssuanceInput::new_issuance(
            asset_amount_sats,
            inflation_amount_sats,
            contract,
        ))
    }

    /// A reissuance of `asset_amount_sats` by `input`, which must hold the blinded reissuance
    /// token of `asset_entropy`.
    fn reissuance(input: &PartialInput, asset_amount_sats: u64, asset_entropy: &str) -> Result<IssuanceInput, JsError> {
        if asset_amount_sats == 0 || i64::try_from(asset_amount_sats).is_err() {
            return Err(JsError::new("reissuance amount must be between 1 and i64::MAX"));
        }

        let entropy = AssetEntropy::from_str(asset_entropy)
            .map_err(|e| JsError::new(&format!("the entropy is not 32 bytes of hex: {e}")))?;
        let secrets = input
            .secrets
            .ok_or_else(|| JsError::new("the token output has no blinding secrets"))?;
        let token = AssetId::reissuance_token_from_entropy(entropy, false);

        if secrets.asset != token {
            return Err(JsError::new(&format!(
                "the output holds {}, not the reissuance token {token}",
                secrets.asset
            )));
        }
        if secrets.asset_bf == AssetBlindingFactor::zero() {
            return Err(JsError::new(
                "the token's asset blinding factor is zero, which Elements reads as a new issuance of another asset",
            ));
        }

        Ok(IssuanceInput::new_reissuance(
            asset_amount_sats,
            entropy.to_byte_array(),
        ))
    }

    /// The issuer contract hash an issuance commits to, all zeroes when none is given.
    fn issuer_contract(written: Option<&str>) -> Result<[u8; 32], String> {
        match written.map(str::trim).filter(|hex_id| !hex_id.is_empty()) {
            Some(hex_id) if hex_id.len() != 64 => Err("a contract hash is 64 hex characters".to_string()),
            Some(hex_id) => ContractHash::from_str(hex_id)
                .map(ContractHash::to_byte_array)
                .map_err(|e| e.to_string()),
            None => Ok([0_u8; 32]),
        }
    }

    fn utxo(txid: &str, vout: u32, tx_out_hex: &str, secrets_json: Option<&str>) -> Result<UTXO, JsError> {
        let outpoint = OutPoint {
            txid: Txid::from_str(txid).map_err(|e| JsError::new(&format!("Invalid txid: {e}")))?,
            vout,
        };

        let bytes = hex::decode(tx_out_hex).map_err(|e| JsError::new(&format!("Invalid output encoding: {e}")))?;
        let txout: TxOut =
            elements::encode::deserialize(&bytes).map_err(|e| JsError::new(&format!("Invalid output: {e}")))?;

        Ok(UTXO {
            outpoint,
            secrets: secrets_json
                .map(Self::blinding_secrets)
                .transpose()
                .map_err(|e| JsError::new(&format!("Invalid blinding secrets: {e}")))?,
            txout,
        })
    }

    fn blinding_secrets(secrets_json: &str) -> Result<TxOutSecrets, String> {
        let parsed: serde_json::Value = serde_json::from_str(secrets_json).map_err(|e| e.to_string())?;

        let field = |name: &str| -> Result<String, String> {
            parsed
                .get(name)
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
                .ok_or_else(|| format!("no \"{name}\""))
        };

        let value = Self::exact_amount(parsed.get("value").ok_or_else(|| "no \"value\"".to_string())?)?;

        Ok(TxOutSecrets::new(
            AssetId::from_str(&field("asset")?).map_err(|e| e.to_string())?,
            AssetBlindingFactor::from_str(&field("assetBlindingFactor")?).map_err(|e| e.to_string())?,
            value,
            ValueBlindingFactor::from_str(&field("valueBlindingFactor")?).map_err(|e| e.to_string())?,
        ))
    }

    /// Reads a safe JSON integer or a decimal string.
    ///
    /// JavaScript may change the decimal digits of numbers above 2^53 - 1 when it
    /// serializes them to JSON. Write larger values as strings containing only
    /// decimal digits with no sign, spaces or leading zeroes.
    fn exact_amount(value: &serde_json::Value) -> Result<u64, String> {
        match value {
            serde_json::Value::Number(number) => {
                let amount = number
                    .as_u64()
                    .ok_or_else(|| format!("{number} is not a whole number of satoshis"))?;

                if amount > 9_007_199_254_740_991 {
                    return Err(format!(
                        "{number} exceeds JavaScript's safe integer limit. Use a decimal string"
                    ));
                }

                Ok(amount)
            }
            serde_json::Value::String(digits) => {
                let canonical = !digits.is_empty()
                    && digits.bytes().all(|byte| byte.is_ascii_digit())
                    && (digits == "0" || !digits.starts_with('0'));

                if !canonical {
                    return Err(format!("\"{digits}\" is not a decimal amount"));
                }

                digits.parse().map_err(|e| format!("\"{digits}\": {e}"))
            }
            other => Err(format!("{other} is not an amount")),
        }
    }

    fn relative_path(path: &str) -> Result<DerivationPath, String> {
        let path = DerivationPath::from_str(path).map_err(|e| e.to_string())?;

        SdkSigner::check_derivation_path(&path).map_err(|e| e.to_string())?;

        Ok(path)
    }

    fn zero_value(ty: &ResolvedType) -> Value {
        let zero = simplicityhl::simplicity::Value::zero(StructuralType::from(ty).as_ref());

        Value::reconstruct(&StructuralValue::from(zero), ty).expect("a zero value has its own type")
    }
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use simplicityhl::elements::confidential::{Asset, AssetBlindingFactor, Value, ValueBlindingFactor};
    use simplicityhl::elements::secp256k1_zkp::SECP256K1;
    use simplicityhl::elements::{
        AssetBlindingNonce, AssetEntropy, AssetId, ContractHash, LockTime, OutPoint, Script, Sequence, TxOut,
        TxOutSecrets, Txid,
    };

    use smplx_sdk::provider::SimplicityNetwork;
    use smplx_sdk::transaction::{IssuanceDetails as SdkIssuanceDetails, PartialInput, UTXO};
    use smplx_sdk::utils::asset_entropy;

    use wasm_bindgen::JsError;

    use crate::issuance_details::IssuanceDetails;
    use crate::program::Program;
    use crate::signer::Signer;

    use super::TransactionBuilder;

    const TRIVIAL: &str = "fn main() { }";

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

    fn report_for(txid: &str, vout: u32, contract: &str) -> IssuanceDetails {
        let outpoint = OutPoint {
            txid: Txid::from_str(txid).expect("a chain vector's txid"),
            vout,
        };
        let entropy = asset_entropy(
            &outpoint,
            TransactionBuilder::issuer_contract(Some(contract)).expect("a chain vector's contract"),
        );

        IssuanceDetails::from_details(&SdkIssuanceDetails {
            asset_id: AssetId::from_entropy(entropy),
            inflation_asset_id: AssetId::reissuance_token_from_entropy(entropy, false),
            asset_entropy: entropy,
        })
    }

    #[test]
    fn issuance_report_matches_liquid_asset_ids() {
        for (txid, vout, contract, asset, _) in ON_CHAIN {
            assert_eq!(report_for(txid, vout, contract).asset_id(), asset);
        }
    }

    #[test]
    fn issuance_report_matches_liquid_token_ids() {
        for (txid, vout, contract, _, token) in ON_CHAIN {
            assert_eq!(report_for(txid, vout, contract).inflation_asset_id(), token);
        }
    }

    #[test]
    fn reported_entropy_derives_the_asset() {
        for (txid, vout, contract, asset, _) in ON_CHAIN {
            let reported = report_for(txid, vout, contract).asset_entropy();
            let read_back = AssetEntropy::from_str(&reported).expect("a reported entropy");

            assert_eq!(AssetId::from_entropy(read_back).to_string(), asset);
        }
    }

    #[test]
    fn missing_issuer_contract_is_all_zeroes() {
        let empty = [0_u8; 32];

        assert_eq!(TransactionBuilder::issuer_contract(None), Ok(empty));
        assert_eq!(TransactionBuilder::issuer_contract(Some("")), Ok(empty));
        assert_eq!(TransactionBuilder::issuer_contract(Some("   ")), Ok(empty));
        assert_eq!(TransactionBuilder::issuer_contract(Some(&"0".repeat(64))), Ok(empty));
    }

    #[test]
    fn issuer_contract_keeps_its_display_byte_order() {
        let written = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";
        let read = TransactionBuilder::issuer_contract(Some(written)).expect("a written id");

        assert_eq!(read[0], 0xd2);
        assert_eq!(read[31], 0xce);
        assert_eq!(ContractHash::from_byte_array(read).to_string(), written);
    }

    #[test]
    fn malformed_issuer_contract_is_rejected() {
        assert!(TransactionBuilder::issuer_contract(Some("not hex at all")).is_err());
        assert!(TransactionBuilder::issuer_contract(Some("00ff")).is_err());
    }

    const SECRETS: &str = r#"{
        "value": 100000,
        "asset": "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49",
        "assetBlindingFactor": "0000000000000000000000000000000000000000000000000000000000000001",
        "valueBlindingFactor": "0000000000000000000000000000000000000000000000000000000000000002"
    }"#;

    #[test]
    fn blinding_secrets_are_parsed() {
        let secrets = TransactionBuilder::blinding_secrets(SECRETS).expect("valid blinding secrets");

        assert_eq!(secrets.value, 100_000);
        assert_eq!(
            secrets.asset.to_string(),
            "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49"
        );
    }

    #[test]
    fn blinding_secrets_without_a_field_are_rejected() {
        for missing in ["value", "asset", "assetBlindingFactor", "valueBlindingFactor"] {
            let without = SECRETS.replace(missing, "somethingElse");

            assert!(
                TransactionBuilder::blinding_secrets(&without).is_err(),
                "secrets without {missing} are rejected"
            );
        }
    }

    #[test]
    fn malformed_blinding_secrets_are_rejected() {
        assert!(TransactionBuilder::blinding_secrets("not json").is_err());
        assert!(TransactionBuilder::blinding_secrets("{}").is_err());
    }

    #[test]
    fn decimal_string_value_is_parsed_exactly() {
        for (written, exact) in [
            ("0", 0),
            ("9007199254740993", 9_007_199_254_740_993),
            ("288230376151711744", 1_u64 << 58),
            ("18446744073709551615", u64::MAX),
        ] {
            let secrets = TransactionBuilder::blinding_secrets(&SECRETS.replace("100000", &format!("\"{written}\"")))
                .expect("a value written as a string");

            assert_eq!(secrets.value, exact, "{written}");
        }
    }

    #[test]
    fn json_number_above_safe_integer_is_rejected() {
        let safe = serde_json::json!(9_007_199_254_740_991_u64);
        let unsafe_number = serde_json::from_str::<serde_json::Value>("288230376151711740")
            .expect("the decimal digits emitted by JSON.stringify(2 ** 58)");

        assert_eq!(TransactionBuilder::exact_amount(&safe), Ok(9_007_199_254_740_991));
        assert!(TransactionBuilder::exact_amount(&unsafe_number).is_err());
        assert_eq!(
            TransactionBuilder::exact_amount(&serde_json::json!("288230376151711744")),
            Ok(1_u64 << 58)
        );
    }

    #[test]
    fn non_decimal_amount_is_rejected() {
        for written in [
            serde_json::json!(""),
            serde_json::json!("+1"),
            serde_json::json!("01"),
            serde_json::json!("1.0"),
            serde_json::json!("18446744073709551616"),
            serde_json::json!(-1),
            serde_json::json!(1.5),
        ] {
            assert!(TransactionBuilder::exact_amount(&written).is_err(), "{written}");
        }
    }

    const TOKEN_BLINDER: &str = "0000000000000000000000000000000000000000000000000000000000000001";
    const VALUE_BLINDER: &str = "0000000000000000000000000000000000000000000000000000000000000003";

    /// The entropy of the first chain vector, in the format `IssuanceDetails` uses.
    fn first_entropy() -> String {
        let (txid, vout, contract, _, _) = ON_CHAIN[0];

        report_for(txid, vout, contract).asset_entropy()
    }

    fn token_of(entropy: &str, confidential_issuance: bool) -> AssetId {
        AssetId::reissuance_token_from_entropy(
            AssetEntropy::from_str(entropy).expect("an entropy"),
            confidential_issuance,
        )
    }

    fn blinder(written: &str) -> AssetBlindingFactor {
        AssetBlindingFactor::from_str(written).expect("a blinder")
    }

    /// An input spending one unit of `asset`, blinded with `committed_bf` or left explicit, with
    /// `claimed_bf` as its secrets.
    fn holding(
        asset: AssetId,
        committed_bf: Option<AssetBlindingFactor>,
        claimed_bf: AssetBlindingFactor,
    ) -> PartialInput {
        let value_bf = ValueBlindingFactor::from_str(VALUE_BLINDER).expect("a blinder");

        let txout = match committed_bf {
            Some(asset_bf) => TxOut {
                asset: Asset::new_confidential(SECP256K1, asset, asset_bf),
                value: Value::new_confidential_from_assetid(SECP256K1, 1, asset, value_bf, asset_bf),
                ..TxOut::default()
            },
            None => TxOut {
                asset: Asset::Explicit(asset),
                value: Value::Explicit(1),
                ..TxOut::default()
            },
        };

        PartialInput::new(UTXO {
            outpoint: OutPoint {
                txid: Txid::from_str(ON_CHAIN[0].0).expect("a chain vector's txid"),
                vout: 0,
            },
            txout,
            secrets: Some(TxOutSecrets::new(asset, claimed_bf, 1, value_bf)),
        })
    }

    #[test]
    fn reissuance_mints_the_original_asset() {
        assert_reissues_the_original_asset(|builder, spent, entropy, secrets| {
            builder.add_reissuance_input(ON_CHAIN[0].0, 0, spent, 5_000, entropy, secrets, None)
        });
    }

    #[test]
    fn program_reissuance_mints_the_original_asset() {
        assert_reissues_the_original_asset(|builder, spent, entropy, secrets| {
            builder.add_program_reissuance_input(
                ON_CHAIN[0].0,
                0,
                spent,
                TRIVIAL,
                None,
                None,
                None,
                5_000,
                entropy,
                secrets,
                None,
                None,
                None,
            )
        });
    }

    /// Asserts that `add` reissues 5000 units of the first chain vector's asset from a blinded
    /// token, given the encoded spent output, the entropy and the token's secrets.
    fn assert_reissues_the_original_asset(
        add: impl FnOnce(&mut TransactionBuilder, &str, &str, &str) -> Result<IssuanceDetails, JsError>,
    ) {
        let entropy = first_entropy();
        let token = token_of(&entropy, false);
        let spent = holding(token, Some(blinder(TOKEN_BLINDER)), blinder(TOKEN_BLINDER));
        let secrets = serde_json::json!({
            "asset": token.to_string(),
            "assetBlindingFactor": TOKEN_BLINDER,
            "value": "1",
            "valueBlindingFactor": VALUE_BLINDER,
        });

        let mut builder = TransactionBuilder::new();
        let report = add(
            &mut builder,
            &hex::encode(simplicityhl::elements::encode::serialize(&spent.witness_utxo)),
            &entropy,
            &secrets.to_string(),
        )
        .expect("a confidential token and its opening");

        assert_eq!(report.asset_id(), ON_CHAIN[0].3);
        assert_eq!(report.inflation_asset_id(), ON_CHAIN[0].4);
        assert_eq!(report.asset_entropy(), entropy);

        // Read the issuance back the way Elements does. A null blinding nonce would make
        // this a new issuance of another asset.
        let (pst, _) = builder.transaction.extract_pst().unwrap();
        let input = &pst.inputs()[0];
        let (asset, minted_token) = input.issuance_ids();

        assert_eq!(
            input.issuance_blinding_nonce,
            Some(AssetBlindingNonce::from_blinding_factor(blinder(TOKEN_BLINDER)))
        );
        assert_eq!(asset.to_string(), ON_CHAIN[0].3);
        assert_eq!(minted_token.to_string(), ON_CHAIN[0].4);
        assert_eq!(input.issuance_value_amount, Some(5_000));
        assert_eq!(input.issuance_inflation_keys, None);
    }

    #[test]
    fn relative_path_is_parsed() {
        assert_eq!(
            TransactionBuilder::relative_path("0/7").expect("a path").to_string(),
            "0/7"
        );
        for path in ["not a path", "", "m", "m/7", "7", "0/7h", "m/84h/1h/0h/0/7"] {
            assert!(TransactionBuilder::relative_path(path).is_err(), "{path}");
        }
    }

    fn spent_output() -> String {
        hex::encode(simplicityhl::elements::encode::serialize(&TxOut::default()))
    }

    fn path_of(builder: &TransactionBuilder, index: usize) -> Option<String> {
        builder.transaction.inputs()[index]
            .partial_input
            .derivation_path
            .as_ref()
            .map(ToString::to_string)
    }

    #[test]
    fn program_input_keeps_its_derivation_path() {
        let mut builder = TransactionBuilder::new();

        let added = builder.add_program_input(
            ON_CHAIN[0].0,
            0,
            &spent_output(),
            TRIVIAL,
            None,
            None,
            Some("SIGNATURE".to_string()),
            None,
            None,
            None,
            Some("0/7".to_string()),
        );

        assert!(added.is_ok());
        assert_eq!(path_of(&builder, 0).as_deref(), Some("0/7"));
    }

    #[test]
    fn program_input_without_path_uses_the_default_key() {
        let mut builder = TransactionBuilder::new();

        let added = builder.add_program_input(
            ON_CHAIN[0].0,
            0,
            &spent_output(),
            TRIVIAL,
            None,
            None,
            Some("SIGNATURE".to_string()),
            None,
            None,
            None,
            None,
        );

        assert!(added.is_ok());
        assert_eq!(path_of(&builder, 0), None);
    }

    #[test]
    fn program_issuance_input_keeps_its_derivation_path() {
        let mut builder = TransactionBuilder::new();

        let added = builder.add_program_issuance_input(
            ON_CHAIN[0].0,
            0,
            &spent_output(),
            TRIVIAL,
            None,
            None,
            Some("SIGNATURE".to_string()),
            1_000,
            0,
            None,
            None,
            None,
            None,
            Some("0/7".to_string()),
        );

        assert!(added.is_ok());
        assert_eq!(path_of(&builder, 0).as_deref(), Some("0/7"));
    }

    #[test]
    fn program_inputs_take_blinding_secrets() {
        let asset = SimplicityNetwork::LiquidTestnet.policy_asset();
        let spent = holding(asset, Some(blinder(TOKEN_BLINDER)), blinder(TOKEN_BLINDER));
        let tx_out_hex = hex::encode(simplicityhl::elements::encode::serialize(&spent.witness_utxo));
        let secrets = serde_json::json!({
            "asset": asset.to_string(),
            "assetBlindingFactor": TOKEN_BLINDER,
            "value": "1",
            "valueBlindingFactor": VALUE_BLINDER,
        })
        .to_string();

        let mut builder = TransactionBuilder::new();

        builder
            .add_program_input(
                ON_CHAIN[0].0,
                0,
                &tx_out_hex,
                TRIVIAL,
                None,
                None,
                None,
                Some(secrets.clone()),
                None,
                None,
                None,
            )
            .unwrap();
        builder
            .add_program_issuance_input(
                ON_CHAIN[0].0,
                1,
                &tx_out_hex,
                TRIVIAL,
                None,
                None,
                None,
                1_000,
                0,
                None,
                Some(secrets),
                None,
                None,
                None,
            )
            .unwrap();

        // Without the secrets, the confidential spent output has no known asset or amount
        for input in builder.transaction.inputs() {
            assert_eq!(input.partial_input.asset_and_amount(), Some((asset, 1)));
        }
    }

    #[test]
    fn native_input_keeps_its_derivation_path() {
        let mut builder = TransactionBuilder::new();

        let added = builder.add_input(ON_CHAIN[0].0, 0, &spent_output(), None, Some("0/7".to_string()));

        assert!(added.is_ok());
        assert_eq!(path_of(&builder, 0).as_deref(), Some("0/7"));
    }

    /// The cold branch reads witness `A`, the hot branch witness `B`.
    const COLD_OR_HOT: &str = r"
        fn main() {
            match witness::PATH {
                Left(cold: ()) => assert!(jet::eq_32(witness::A, 0)),
                Right(hot: ()) => assert!(jet::eq_32(witness::B, 0)),
            }
        }
    ";

    #[test]
    fn unused_witness_lets_the_other_branch_be_spent() {
        let network = "liquidtestnet";
        let policy = SimplicityNetwork::LiquidTestnet.policy_asset();
        let script_hex = Program::new(COLD_OR_HOT, None, None, None)
            .and_then(|program| program.script_pubkey(network))
            .unwrap();
        let spent = TxOut {
            asset: Asset::Explicit(policy),
            value: Value::Explicit(50_000),
            script_pubkey: Script::from(hex::decode(&script_hex).unwrap()),
            ..TxOut::default()
        };
        let cold = r#"{"PATH": {"value": "Left(())", "type": "Either<(), ()>"}, "A": {"value": "0", "type": "u32"}}"#;

        let mut builder = TransactionBuilder::new();

        builder
            .add_program_input(
                ON_CHAIN[0].0,
                0,
                &hex::encode(simplicityhl::elements::encode::serialize(&spent)),
                COLD_OR_HOT,
                None,
                Some(cold.to_string()),
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        builder
            .add_output(&script_hex, 49_000, &policy.to_string(), None)
            .unwrap();
        builder.set_unused_witnesses(0, vec!["B".to_string()]).unwrap();

        assert!(Signer::estimate_strict_fee(&builder, network, 1000.0).is_ok());
    }

    #[test]
    fn removing_shifts_the_later_inputs_and_outputs_down() {
        let mut builder = TransactionBuilder::new();
        let policy = SimplicityNetwork::LiquidTestnet.policy_asset().to_string();

        builder
            .add_input(ON_CHAIN[0].0, 0, &spent_output(), None, None)
            .unwrap();
        builder
            .add_input(ON_CHAIN[0].0, 1, &spent_output(), None, None)
            .unwrap();
        builder.add_output("", 1_000, &policy, None).unwrap();
        builder.add_output("", 2_000, &policy, None).unwrap();

        builder.remove_input(0).unwrap();
        builder.remove_output(0).unwrap();

        assert_eq!(builder.input_count(), 1);
        assert_eq!(builder.transaction.inputs()[0].partial_input.outpoint.vout, 1);
        assert_eq!(builder.output_count(), 1);
        assert_eq!(builder.transaction.outputs()[0].amount, 2_000);
    }

    #[test]
    fn input_locktime_overrides_the_transaction_locktime() {
        let mut builder = TransactionBuilder::new();

        builder
            .add_input(ON_CHAIN[0].0, 0, &spent_output(), None, None)
            .unwrap();
        builder
            .add_input(ON_CHAIN[0].0, 1, &spent_output(), None, None)
            .unwrap();
        builder.set_locktime_height(100);
        builder.set_input_locktime_height(0, 200).unwrap();

        let locktime_of = |index: usize| builder.transaction.inputs()[index].partial_input.locktime;

        assert_eq!(locktime_of(0), Some(LockTime::from_height(200).unwrap()));
        assert_eq!(locktime_of(1), None);
        assert_eq!(builder.transaction.sequence(), Sequence::ENABLE_LOCKTIME_NO_RBF);
    }

    #[test]
    fn input_sequence_overrides_the_transaction_sequence() {
        let mut builder = TransactionBuilder::new();

        builder
            .add_input(ON_CHAIN[0].0, 0, &spent_output(), None, None)
            .unwrap();
        builder
            .add_input(ON_CHAIN[0].0, 1, &spent_output(), None, None)
            .unwrap();
        builder.set_sequence(1);
        builder.set_input_sequence(0, 25_920).unwrap();

        let sequence_of = |index: usize| builder.transaction.inputs()[index].partial_input.sequence;

        assert_eq!(sequence_of(0), Some(Sequence::from_consensus(25_920)));
        assert_eq!(sequence_of(1), None);
        assert_eq!(builder.transaction.sequence(), Sequence::from_consensus(1));
    }
}
