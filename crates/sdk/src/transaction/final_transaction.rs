use std::collections::HashMap;

use bitcoin_hashes::sha256;

use simplicityhl::elements::pset::{Input, PartiallySignedTransaction};
use simplicityhl::elements::{
    AssetId, TxOutSecrets,
    confidential::{AssetBlindingFactor, ValueBlindingFactor},
};

use crate::provider::SimplicityNetwork;
use crate::utils;

use super::partial_input::{IssuanceInput, PartialInput, ProgramInput, RequiredSignature};
use super::partial_output::PartialOutput;

/// Constant is defined for fee calculation on transaction sending.
pub const WITNESS_SCALE_FACTOR: usize = 4;

/// A structure representing the details of token issuance and related metadata.
#[derive(Debug, Clone)]
pub struct IssuanceDetails {
    /// The unique `AssetId` generated from the provided entropy, representing the issued tokens struct.
    pub asset_id: AssetId,
    /// The `AssetId` corresponding to the reissuance (inflation) token, used for minting new tokens.
    pub inflation_asset_id: AssetId,
    /// The entropy value (`sha256::Midstate`) that was used to derive both the `asset_id` and `inflation_asset_id`.
    pub asset_entropy: sha256::Midstate,
}

/// Represents the final input structure put into a `FinalTransaction` for processing.
#[derive(Clone)]
pub struct FinalInput {
    /// Holds the base input data required for the operation.
    pub partial_input: PartialInput,
    /// Holds program inputs, which are used for program witness finalization.
    pub program_input: Option<ProgramInput>,
    /// Contains optional issuance-related information.
    pub issuance_input: Option<IssuanceInput>,
    /// Required signature for finalizing the transaction.
    pub required_sig: RequiredSignature,
}

impl FinalInput {
    /// Creates a new instance of the type with the specified `partial_input` and `required_sig`.
    #[must_use]
    pub fn new(partial_input: PartialInput, required_sig: RequiredSignature) -> Self {
        Self {
            partial_input,
            required_sig,
            program_input: None,
            issuance_input: None,
        }
    }

    /// Sets the `program_input` field with the given `ProgramInput` and returns the modified `FinalInput`.
    #[must_use]
    pub fn with_program(mut self, program_input: ProgramInput) -> Self {
        self.program_input = Some(program_input);

        self
    }

    /// Sets the `issuance_input` field of the current instance and returns the updated `FinalInput`.
    #[must_use]
    pub fn with_issuance(mut self, issuance_input: IssuanceInput) -> Self {
        self.issuance_input = Some(issuance_input);

        self
    }

    /// Retrieves the issuance details associated with the current instance.
    ///
    /// # Errors
    ///
    /// This method does not explicitly return errors but returns `None` if no issuance
    /// input is available.
    #[must_use]
    pub fn get_issuance_details(&self) -> Option<IssuanceDetails> {
        match &self.issuance_input {
            Some(issuance_input) => {
                let asset_entropy = match issuance_input {
                    IssuanceInput::Issuance { asset_entropy, .. } => {
                        utils::asset_entropy(&self.partial_input.outpoint(), *asset_entropy)
                    }
                    IssuanceInput::Reissuance { asset_entropy, .. } => {
                        sha256::Midstate::from_byte_array(*asset_entropy)
                    }
                };

                let asset_id = AssetId::from_entropy(asset_entropy);
                let inflation_asset_id = AssetId::reissuance_token_from_entropy(asset_entropy, false);

                Some(IssuanceDetails {
                    asset_id,
                    inflation_asset_id,
                    asset_entropy,
                })
            }
            None => None,
        }
    }

    /// Converts the current object into an `Input` representation, including any
    /// issuance input and partial input details.
    ///
    /// # Panics
    ///
    /// This function will panic if the `issuance_input` is of type `Reissuance`
    ///  and the `partial_input.secrets` field is `None` or does not contain the necessary
    ///  confidential information. Specifically, a panic occurs when attempting to unwrap the `asset_bf` value.
    #[must_use]
    pub fn to_input(&self) -> Input {
        let mut pst_input = self.partial_input.to_input();

        // populate the input manually since `input.merge` is private
        if let Some(issuance_input) = &self.issuance_input {
            let issue = issuance_input.to_input();

            pst_input.issuance_value_amount = issue.issuance_value_amount;
            pst_input.issuance_asset_entropy = issue.issuance_asset_entropy;
            pst_input.issuance_inflation_keys = issue.issuance_inflation_keys;
            pst_input.blinded_issuance = issue.blinded_issuance;

            if matches!(issuance_input, IssuanceInput::Reissuance { .. }) {
                let issuance_blinding_nonce = self
                    .partial_input
                    .secrets
                    .expect("Reissuance input must be confidential")
                    .asset_bf
                    .into_inner();

                pst_input.issuance_blinding_nonce = Some(issuance_blinding_nonce);
            }
        }

        pst_input
    }
}

/// A struct representing a final (but not yet signed) transaction.
#[derive(Clone)]
pub struct FinalTransaction {
    inputs: Vec<FinalInput>,
    outputs: Vec<PartialOutput>,
}

impl FinalTransaction {
    /// Creates a new instance of the final transaction with default values.
    #[must_use]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// Adds a new input to the transaction.
    ///
    /// # Panics
    /// Panics if the requested signature is not `NativeEcdsa` or `None`.
    /// (i.e. if `required_sig` is `RequiredSignature::Witness` or `RequiredSignature::WitnessWithPath`)
    pub fn add_input(&mut self, partial_input: PartialInput, required_sig: RequiredSignature) {
        match required_sig {
            RequiredSignature::Witness(_) | RequiredSignature::WitnessWithPath(_, _) => {
                panic!("Requested signature is not NativeEcdsa or None")
            }
            _ => {}
        }

        self.push_new_input(FinalInput::new(partial_input, required_sig));
    }

    /// Adds a new program input to the transaction.
    ///
    /// # Panics
    /// The function will panic if the `required_sig` parameter is of type `RequiredSignature::NativeEcdsa`,
    /// as this type of signature is not applicable for program inputs.
    pub fn add_program_input(
        &mut self,
        partial_input: PartialInput,
        program_input: ProgramInput,
        required_sig: RequiredSignature,
    ) {
        if let RequiredSignature::NativeEcdsa = required_sig {
            panic!("Requested signature is not Witness or None");
        }

        self.push_new_input(FinalInput::new(partial_input, required_sig).with_program(program_input));
    }

    /// Adds an issuance (or reissuance) input to the transaction.
    ///
    /// # Panics
    /// This function panics if the `required_sig` is of type `Witness` or
    /// `WitnessWithPath`, as these signature types are not allowed in the current context.
    pub fn add_issuance_input(
        &mut self,
        partial_input: PartialInput,
        issuance_input: IssuanceInput,
        required_sig: RequiredSignature,
    ) -> IssuanceDetails {
        match required_sig {
            RequiredSignature::Witness(_) | RequiredSignature::WitnessWithPath(_, _) => {
                panic!("Requested signature is not NativeEcdsa or None")
            }
            _ => {}
        }

        self.push_new_input(FinalInput::new(partial_input, required_sig).with_issuance(issuance_input))
            .unwrap()
    }

    /// Adds an issuance program input to the transaction with the specified parameters.
    ///
    /// # Panics
    /// Panics if the `required_sig` parameter is of type `RequiredSignature::NativeEcdsa`.
    /// Also panics if the populated input fails to return valid issuance details.
    pub fn add_program_issuance_input(
        &mut self,
        partial_input: PartialInput,
        program_input: ProgramInput,
        issuance_input: IssuanceInput,
        required_sig: RequiredSignature,
    ) -> IssuanceDetails {
        if let RequiredSignature::NativeEcdsa = required_sig {
            panic!("Requested signature is not Witness or None");
        }

        self.push_new_input(
            FinalInput::new(partial_input, required_sig)
                .with_program(program_input)
                .with_issuance(issuance_input),
        )
        .unwrap()
    }

    /// Removes an input from the list of inputs at the specified index.
    pub fn remove_input(&mut self, index: usize) -> Option<FinalInput> {
        if self.inputs.get(index).is_some() {
            return Some(self.inputs.remove(index));
        }

        None
    }

    /// Adds a partial output to the list of outputs.
    pub fn add_output(&mut self, partial_output: PartialOutput) {
        self.outputs.push(partial_output);
    }

    /// Removes an output from the `outputs` list at the specified index.
    ///
    /// # Panics
    /// This function does not panic. If the `index` is invalid, it will return `None` instead of causing a panic.
    pub fn remove_output(&mut self, index: usize) -> Option<PartialOutput> {
        if self.outputs.get(index).is_some() {
            return Some(self.outputs.remove(index));
        }

        None
    }

    /// Provides a slice reference to the collection of `FinalInput` elements.
    #[must_use]
    pub fn inputs(&self) -> &[FinalInput] {
        &self.inputs
    }

    /// Provides mutable access to the `inputs` field.
    ///
    /// This method returns a mutable slice of `FinalInput` elements,
    /// allowing the caller to modify the elements in the `inputs` field.
    pub fn inputs_mut(&mut self) -> &mut [FinalInput] {
        &mut self.inputs
    }

    /// Returns a reference to the slice of `PartialOutput` elements contained within the struct.
    #[must_use]
    pub fn outputs(&self) -> &[PartialOutput] {
        &self.outputs
    }

    /// Provides mutable access to the `outputs` field of the current struct.
    pub fn outputs_mut(&mut self) -> &mut [PartialOutput] {
        &mut self.outputs
    }

    /// Returns the number of inputs associated with the current instance.
    #[must_use]
    pub fn n_inputs(&self) -> usize {
        self.inputs.len()
    }

    /// Returns the number of outputs associated with the object.
    #[must_use]
    pub fn n_outputs(&self) -> usize {
        self.outputs.len()
    }

    /// Checks if any of the outputs require blinding, determines if at least one of them has a `blinding_key` specified.
    #[must_use]
    pub fn needs_blinding(&self) -> bool {
        self.outputs.iter().any(|el| el.blinding_key.is_some())
    }

    /// Calculates the fee delta for a transaction based on the inputs and outputs.
    ///
    /// The fee delta represents the net difference between the available asset amount
    /// from the transaction's inputs and the consumed asset amount by its outputs.
    /// The function considers the network's policy asset to determine which inputs
    /// and outputs contribute to the calculation.
    ///
    /// # Panics
    /// Function will panic if the asset doesn't be unblinded correctly, and PST input asset and amount is confidential.
    #[must_use]
    pub fn calculate_fee_delta(&self, network: &SimplicityNetwork) -> i64 {
        let mut available_amount = 0;

        for input in &self.inputs {
            match input.partial_input.secrets {
                // this is an unblinded confidential input
                Some(secrets) => {
                    if secrets.asset == network.policy_asset() {
                        available_amount += secrets.value;
                    }
                }
                // this is an explicit input
                None => {
                    if input.partial_input.asset.unwrap() == network.policy_asset() {
                        available_amount += input.partial_input.amount.unwrap();
                    }
                }
            }
        }

        let consumed_amount = self
            .outputs
            .iter()
            .filter(|output| output.asset == network.policy_asset())
            .fold(0_u64, |acc, output| acc + output.amount);

        available_amount.cast_signed() - consumed_amount.cast_signed()
    }

    /// Computes the transaction fee based on the provided weight and fee rate.
    ///
    /// Overall, the function calculates the virtual size (vsize) of the transaction as:
    /// `weight / WITNESS_SCALE_FACTOR`, rounded up to the nearest whole number.
    /// Then, the fee is computed as `(vsize * fee_rate / 1000.0)`, also rounded up.
    ///
    /// # Returns
    /// The transaction fee in satoshis, rounded up to the nearest whole number.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    #[must_use]
    pub fn calculate_fee(&self, weight: usize, fee_rate: f32) -> u64 {
        let vsize = weight.div_ceil(WITNESS_SCALE_FACTOR);

        (vsize as f32 * fee_rate / 1000.0).ceil() as u64
    }

    /// Extracts a partially signed transaction (PST) and a mapping of input secrets from the current state.
    ///
    /// # Panics
    /// Function will panic if the pst input is a confidential issuance.
    #[must_use]
    pub fn extract_pst(&self) -> (PartiallySignedTransaction, HashMap<usize, TxOutSecrets>) {
        let mut input_secrets = HashMap::new();
        let mut pst = PartiallySignedTransaction::new_v2();

        for i in 0..self.inputs.len() {
            let final_input = &self.inputs[i];
            let pst_input = final_input.to_input();

            match final_input.partial_input.secrets {
                // insert input secrets if present
                Some(secrets) => input_secrets.insert(i, secrets),
                // else populate input secrets with "explicit" amounts
                None => input_secrets.insert(
                    i,
                    TxOutSecrets {
                        asset: pst_input.asset.unwrap(),
                        asset_bf: AssetBlindingFactor::zero(),
                        value: pst_input.amount.unwrap(),
                        value_bf: ValueBlindingFactor::zero(),
                    },
                ),
            };

            pst.add_input(pst_input);
        }

        self.outputs.iter().for_each(|el| {
            pst.add_output(el.to_output());
        });

        (pst, input_secrets)
    }

    fn push_new_input(&mut self, new_input: FinalInput) -> Option<IssuanceDetails> {
        let issuance_details = new_input.get_issuance_details();

        self.inputs.push(new_input);

        issuance_details
    }
}

#[cfg(test)]
mod tests {
    use bitcoin_hashes::Hash;

    use simplicityhl::elements::{OutPoint, Script, TxOut, Txid};

    use crate::transaction::UTXO;

    use super::*;

    fn dummy_asset_id(byte: u8) -> AssetId {
        AssetId::from_slice(&[byte; 32]).unwrap()
    }

    fn dummy_txid(byte: u8) -> Txid {
        Txid::from_slice(&[byte; 32]).unwrap()
    }

    fn explicit_utxo(txid_byte: u8, vout: u32, amount: u64, asset: AssetId) -> UTXO {
        UTXO {
            outpoint: OutPoint::new(dummy_txid(txid_byte), vout),
            txout: TxOut::new_fee(amount, asset),
            secrets: None,
        }
    }

    fn confidential_utxo(txid_byte: u8, vout: u32, asset: AssetId, value: u64) -> UTXO {
        UTXO {
            outpoint: OutPoint::new(dummy_txid(txid_byte), vout),
            txout: TxOut::default(),
            secrets: Some(TxOutSecrets::new(
                asset,
                AssetBlindingFactor::zero(),
                value,
                ValueBlindingFactor::zero(),
            )),
        }
    }

    // Manually construct PST and check extract_pst correctness based on it
    #[test]
    fn extract_pst_single_explicit_input_single_output() {
        let policy = dummy_asset_id(0xAA);

        let utxo = explicit_utxo(0x01, 0, 5000, policy);
        let partial_input = PartialInput::new(utxo);
        let partial_output = PartialOutput::new(Script::new(), 4000, policy);

        let mut ft = FinalTransaction::new();
        ft.add_input(partial_input.clone(), RequiredSignature::None);
        ft.add_output(partial_output.clone());

        let mut expected_pst = PartiallySignedTransaction::new_v2();
        expected_pst.add_input(partial_input.to_input());
        expected_pst.add_output(partial_output.to_output());

        let expected_secrets: HashMap<usize, TxOutSecrets> = HashMap::from([(
            0,
            TxOutSecrets::new(policy, AssetBlindingFactor::zero(), 5000, ValueBlindingFactor::zero()),
        )]);

        let (pst, secrets) = ft.extract_pst();

        assert_eq!(pst, expected_pst);
        assert_eq!(secrets, expected_secrets);
    }

    #[test]
    fn extract_pst_single_confidential_input() {
        let policy = dummy_asset_id(0xAA);

        let utxo = confidential_utxo(0x01, 0, policy, 3000);
        let partial_input = PartialInput::new(utxo);
        let partial_output = PartialOutput::new(Script::new(), 2000, policy);

        let mut ft = FinalTransaction::new();
        ft.add_input(partial_input.clone(), RequiredSignature::None);
        ft.add_output(partial_output.clone());

        let mut expected_pst = PartiallySignedTransaction::new_v2();
        expected_pst.add_input(partial_input.to_input());
        expected_pst.add_output(partial_output.to_output());

        let expected_secrets = HashMap::from([(
            0,
            TxOutSecrets::new(policy, AssetBlindingFactor::zero(), 3000, ValueBlindingFactor::zero()),
        )]);

        let (pst, secrets) = ft.extract_pst();

        assert_eq!(pst, expected_pst);
        assert_eq!(secrets, expected_secrets);
    }

    #[test]
    fn extract_pst_mixed_inputs_multiple_outputs() {
        let policy = dummy_asset_id(0xAA);
        let other = dummy_asset_id(0xBB);

        let explicit_utxo = explicit_utxo(0x01, 0, 5000, policy);
        let conf_utxo = confidential_utxo(0x02, 1, other, 1000);

        let explicit_partial = PartialInput::new(explicit_utxo);
        let conf_partial = PartialInput::new(conf_utxo);

        let output_a = PartialOutput::new(Script::new(), 3000, policy);
        let output_b = PartialOutput::new(Script::new(), 800, other);

        let mut ft = FinalTransaction::new();
        ft.add_input(explicit_partial.clone(), RequiredSignature::None);
        ft.add_input(conf_partial.clone(), RequiredSignature::None);
        ft.add_output(output_a.clone());
        ft.add_output(output_b.clone());

        let mut expected_pst = PartiallySignedTransaction::new_v2();
        expected_pst.add_input(explicit_partial.to_input());
        expected_pst.add_input(conf_partial.to_input());
        expected_pst.add_output(output_a.to_output());
        expected_pst.add_output(output_b.to_output());

        let expected_secrets = HashMap::from([
            (
                0,
                TxOutSecrets::new(policy, AssetBlindingFactor::zero(), 5000, ValueBlindingFactor::zero()),
            ),
            (
                1,
                TxOutSecrets::new(other, AssetBlindingFactor::zero(), 1000, ValueBlindingFactor::zero()),
            ),
        ]);

        let (pst, secrets) = ft.extract_pst();

        assert_eq!(pst, expected_pst);
        assert_eq!(secrets, expected_secrets);
    }

    #[test]
    fn extract_pst_with_issuance_input() {
        let policy = dummy_asset_id(0xAA);
        let entropy = [0x42u8; 32];
        let issuance_amount = 1_000_000u64;

        let utxo = explicit_utxo(0x01, 0, 5000, policy);
        let partial_input = PartialInput::new(utxo);
        let issuance = IssuanceInput::new_issuance(issuance_amount, 0, entropy);
        let partial_output = PartialOutput::new(Script::new(), 4000, policy);

        let mut ft = FinalTransaction::new();
        ft.add_issuance_input(partial_input.clone(), issuance.clone(), RequiredSignature::None);
        ft.add_output(partial_output.clone());

        // build expected pst, merge partial_input and issuance manually
        let mut expected_pst = PartiallySignedTransaction::new_v2();
        let mut expected_input = partial_input.to_input();
        let issuance_input = issuance.to_input();
        expected_input.issuance_value_amount = issuance_input.issuance_value_amount;
        expected_input.issuance_asset_entropy = issuance_input.issuance_asset_entropy;
        expected_input.issuance_inflation_keys = issuance_input.issuance_inflation_keys;
        expected_input.issuance_blinding_nonce = None;
        expected_input.blinded_issuance = issuance_input.blinded_issuance;
        expected_pst.add_input(expected_input);
        expected_pst.add_output(partial_output.to_output());

        let expected_secrets = HashMap::from([(
            0,
            TxOutSecrets::new(policy, AssetBlindingFactor::zero(), 5000, ValueBlindingFactor::zero()),
        )]);

        let (pst, secrets) = ft.extract_pst();

        assert_eq!(pst, expected_pst);
        assert_eq!(secrets, expected_secrets);
    }

    #[test]
    fn extract_pst_with_reissuance_input() {
        let policy = dummy_asset_id(0xAA);
        let entropy = [0x42u8; 32];
        let issuance_amount = 1_000_000u64;

        let conf_utxo = confidential_utxo(0x02, 0, policy, 1000);
        let partial_input = PartialInput::new(conf_utxo);
        let reissuance_input = IssuanceInput::new_reissuance(issuance_amount, entropy);
        let partial_output = PartialOutput::new(Script::new(), 1000, policy);

        let mut ft = FinalTransaction::new();
        ft.add_issuance_input(partial_input.clone(), reissuance_input.clone(), RequiredSignature::None);
        ft.add_output(partial_output.clone());

        // build expected pst, merge partial_input and issuance manually
        let mut expected_pst = PartiallySignedTransaction::new_v2();
        let mut expected_input = partial_input.to_input();
        let issuance_input = reissuance_input.to_input();
        expected_input.issuance_value_amount = issuance_input.issuance_value_amount;
        expected_input.issuance_asset_entropy = issuance_input.issuance_asset_entropy;
        expected_input.issuance_inflation_keys = None;
        expected_input.issuance_blinding_nonce = Some(partial_input.secrets.unwrap().asset_bf.into_inner());
        expected_input.blinded_issuance = issuance_input.blinded_issuance;
        expected_pst.add_input(expected_input);
        expected_pst.add_output(partial_output.to_output());

        let expected_secrets = HashMap::from([(
            0,
            TxOutSecrets::new(policy, AssetBlindingFactor::zero(), 1000, ValueBlindingFactor::zero()),
        )]);

        let (pst, secrets) = ft.extract_pst();

        assert_eq!(pst, expected_pst);
        assert_eq!(secrets, expected_secrets);
    }
}
