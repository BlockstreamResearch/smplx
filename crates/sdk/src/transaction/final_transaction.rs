use std::collections::HashMap;

use simplicityhl::elements::pset::{Input, PartiallySignedTransaction};
use simplicityhl::elements::{
    AssetBlindingNonce, AssetId, LockTime, Sequence, TxOutSecrets,
    confidential::{AssetBlindingFactor, ValueBlindingFactor},
};

use crate::provider::SimplicityNetwork;

use super::change_output::ChangeOutput;
use super::error::FinalTransactionError;
use super::partial_input::{IssuanceDetails, IssuanceInput, PartialInput, ProgramInput};
use super::partial_output::PartialOutput;
use super::required_signature::{NativeSignature, ProgramSignature, RequiredSignature};

/// Represents the final input structure put into a `FinalTransaction` for processing.
#[derive(Clone, Debug)]
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
        self.issuance_input
            .as_ref()
            .map(|issuance_input| issuance_input.details(&self.partial_input.outpoint))
    }

    /// Returns the input's asset ID and amount from unblinding secrets, or from
    /// explicit fields when no secrets are available.
    ///
    /// Returns `None` if no secrets are available and either explicit field is missing.
    #[must_use]
    pub fn get_asset_and_amount(&self) -> Option<(AssetId, u64)> {
        self.partial_input.asset_and_amount()
    }

    /// Converts the current object into an `Input` representation, including any
    /// issuance input and partial input details.
    ///
    /// # Panics
    ///
    /// This function will panic if the `issuance_input` is of type `Reissuance`
    /// and the `partial_input.secrets` field is `None` or does not contain the necessary
    /// confidential information. Specifically, a panic occurs when attempting to unwrap the `asset_bf` value.
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
                let issuance_blinding_nonce = AssetBlindingNonce::from_blinding_factor(
                    self.partial_input
                        .secrets
                        .expect("Reissuance input must be confidential")
                        .asset_bf,
                );

                pst_input.issuance_blinding_nonce = Some(issuance_blinding_nonce);
            }
        }

        pst_input
    }
}

/// A struct representing a final (but not yet signed) transaction.
#[derive(Clone, Debug)]
pub struct FinalTransaction {
    inputs: Vec<FinalInput>,
    outputs: Vec<PartialOutput>,
    change: Option<ChangeOutput>,
    sequence: Option<Sequence>,
    locktime: LockTime,
}

impl FinalTransaction {
    /// Creates a new instance of the final transaction with default values.
    #[must_use]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            change: None,
            sequence: None,
            locktime: LockTime::ZERO,
        }
    }

    /// Sets a specific `Sequence` for the transaction.
    ///
    /// Injects this value into the inputs that don't declare their own sequence.
    pub fn set_sequence(&mut self, sequence: Sequence) {
        self.sequence = Some(sequence);
    }

    /// Sets a specific `LockTime` for the transaction.
    ///
    /// Injects this value into the inputs that don't declare their own locktime.
    pub fn set_locktime(&mut self, locktime: LockTime) {
        self.locktime = locktime;
    }

    /// The sequence of the inputs that don't declare their own. The one set with `set_sequence`,
    /// or `ENABLE_LOCKTIME_NO_RBF` when a locktime is set to be enforced, and `MAX` otherwise.
    #[must_use]
    pub fn sequence(&self) -> Sequence {
        self.sequence.unwrap_or(if self.has_locktime() {
            Sequence::ENABLE_LOCKTIME_NO_RBF
        } else {
            Sequence::MAX
        })
    }

    /// The locktime of the the transaction, e.g. inputs that don't declare their own.
    #[must_use]
    pub fn locktime(&self) -> LockTime {
        self.locktime
    }

    /// Sets where this transaction's change should go.
    ///
    /// Left unset, the signer sends change to the single address it derives internally.
    pub fn add_change(&mut self, change: ChangeOutput) {
        self.change = Some(change);
    }

    /// Drops the change target, returning to the signer's own address.
    pub fn remove_change(&mut self) {
        self.change = None;
    }

    /// Adds a native input to the transaction.
    pub fn add_input(&mut self, partial_input: PartialInput, signature: Option<NativeSignature>) {
        self.inputs.push(FinalInput::new(partial_input, signature.into()));
    }

    /// Adds a program input to the transaction, with the signature the signer injects into its witness, if any.
    pub fn add_program_input(
        &mut self,
        partial_input: PartialInput,
        program_input: ProgramInput,
        signature: Option<ProgramSignature>,
    ) {
        self.inputs
            .push(FinalInput::new(partial_input, signature.into()).with_program(program_input));
    }

    /// Adds a native issuance (or reissuance) input to the transaction.
    pub fn add_issuance_input(
        &mut self,
        partial_input: PartialInput,
        issuance_input: IssuanceInput,
        signature: Option<NativeSignature>,
    ) -> IssuanceDetails {
        let details = issuance_input.details(&partial_input.outpoint);

        self.inputs
            .push(FinalInput::new(partial_input, signature.into()).with_issuance(issuance_input));

        details
    }

    /// Adds a program issuance (or reissuance) input to the transaction.
    pub fn add_program_issuance_input(
        &mut self,
        partial_input: PartialInput,
        program_input: ProgramInput,
        issuance_input: IssuanceInput,
        signature: Option<ProgramSignature>,
    ) -> IssuanceDetails {
        let details = issuance_input.details(&partial_input.outpoint);

        self.inputs.push(
            FinalInput::new(partial_input, signature.into())
                .with_program(program_input)
                .with_issuance(issuance_input),
        );

        details
    }

    /// Removes the input at `index`, `None` when there is none.
    pub fn remove_input(&mut self, index: usize) -> Option<FinalInput> {
        (index < self.inputs.len()).then(|| self.inputs.remove(index))
    }

    /// Adds a partial output to the list of outputs.
    pub fn add_output(&mut self, partial_output: PartialOutput) {
        self.outputs.push(partial_output);
    }

    /// Removes the output at `index`, `None` when there is none.
    pub fn remove_output(&mut self, index: usize) -> Option<PartialOutput> {
        (index < self.outputs.len()).then(|| self.outputs.remove(index))
    }

    /// Where this transaction's change should go, when the caller said.
    #[must_use]
    pub fn change(&self) -> Option<&ChangeOutput> {
        self.change.as_ref()
    }

    /// Where this transaction's change should go, for a caller that needs to amend it.
    #[must_use]
    pub fn change_mut(&mut self) -> Option<&mut ChangeOutput> {
        self.change.as_mut()
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

    /// Checks whether any input being spent is confidential.
    ///
    /// Blinding balances the inputs against the outputs, so a transaction spending a confidential input needs
    /// at least one blinded output to balance against. Left without one it is rejected by the node as `bad-txns-in-ne-out`.
    #[must_use]
    pub fn has_confidential_input(&self) -> bool {
        self.inputs.iter().any(|el| {
            el.partial_input.witness_utxo.value.is_confidential()
                || el.partial_input.witness_utxo.asset.is_confidential()
                || el.partial_input.secrets.is_some()
        })
    }

    /// Calculates the fee delta for a transaction based on the inputs and outputs.
    ///
    /// The fee delta represents the net difference between the available asset amount
    /// from the transaction's inputs and the consumed asset amount by its outputs.
    /// The function considers the network's policy asset to determine which inputs
    /// and outputs contribute to the calculation.
    ///
    /// # Errors
    /// Returns [`FinalTransactionError::MissingInputSecrets`] if an input's asset or
    /// amount is unknown and no unblinding secrets are available. A visible amount
    /// alone does not identify its asset.
    pub fn calculate_fee_delta(&self, network: &SimplicityNetwork) -> Result<i64, FinalTransactionError> {
        let mut available_amount = 0;

        for (input_index, input) in self.inputs.iter().enumerate() {
            let (asset, amount) = input
                .get_asset_and_amount()
                .ok_or(FinalTransactionError::MissingInputSecrets { input_index })?;
            if asset == network.policy_asset() {
                available_amount += amount;
            }
        }

        let consumed_amount = self
            .outputs
            .iter()
            .filter(|output| output.asset == network.policy_asset())
            .fold(0_u64, |acc, output| acc + output.amount);

        Ok(available_amount.cast_signed() - consumed_amount.cast_signed())
    }

    /// Checks if the transaction is balanced, meaning all inputs - all outputs = 0 for every asset.
    ///
    /// Issued and reissued amounts are credited to the input that declares them, so a newly created
    /// asset is balanced against its declared `issuance_amount`/`inflation_amount`.
    ///
    /// # Errors
    /// Returns [`FinalTransactionError::MissingInputSecrets`] if an input's asset or
    /// amount is unknown and no unblinding secrets are available.
    pub fn is_balanced(&self) -> Result<bool, FinalTransactionError> {
        let mut transfers: HashMap<AssetId, i64> = HashMap::new();

        // Collecting all inputs
        for (input_index, input) in self.inputs.iter().enumerate() {
            let (asset, amount) = input
                .get_asset_and_amount()
                .ok_or(FinalTransactionError::MissingInputSecrets { input_index })?;

            let transfer_entry = transfers.entry(asset).or_insert(0);
            *transfer_entry += amount.cast_signed();

            // Issuance brings new assets into the transaction, so the declared amounts count as inputs.
            // A reissuance mints no new inflation tokens
            if let Some(issuance_details) = input.get_issuance_details()
                && let Some(issuance_input) = &input.issuance_input
            {
                let (issuance_amount, inflation_amount) = match issuance_input {
                    IssuanceInput::Issuance {
                        issuance_amount,
                        inflation_amount,
                        ..
                    } => (*issuance_amount, *inflation_amount),
                    IssuanceInput::Reissuance { issuance_amount, .. } => (*issuance_amount, 0),
                };

                *transfers.entry(issuance_details.asset_id).or_insert(0) += issuance_amount.cast_signed();
                *transfers.entry(issuance_details.inflation_asset_id).or_insert(0) += inflation_amount.cast_signed();
            }
        }

        for output in &self.outputs {
            if output.script_pubkey.is_op_return() && output.amount == 0 {
                continue;
            }

            match transfers.get_mut(&output.asset) {
                Some(value) => {
                    *value -= output.amount.cast_signed();
                }
                None => return Ok(false),
            }
        }

        // All transfers including fee should sum up to 0
        Ok(transfers.values().all(|&value| value == 0))
    }

    /// Extracts a partially signed transaction (PST) and a mapping of input secrets from the current state.
    ///
    /// # Errors
    /// Returns [`FinalTransactionError::MissingInputSecrets`] if an input's asset or
    /// amount is unknown and no unblinding secrets are available.
    ///
    /// # Panics
    /// Panics if a reissuance input has no blinding secrets.
    pub fn extract_pst(
        &self,
    ) -> Result<(PartiallySignedTransaction, HashMap<usize, TxOutSecrets>), FinalTransactionError> {
        let mut input_secrets = HashMap::new();
        let mut pst = PartiallySignedTransaction::new_v2();
        let sequence = self.sequence();

        for i in 0..self.inputs.len() {
            let mut final_input = self.inputs[i].clone();

            // An input that sets no sequence or locktime takes the transaction's
            final_input.partial_input.sequence.get_or_insert(sequence);
            final_input.partial_input.locktime.get_or_insert(self.locktime);

            let secrets = if let Some(secrets) = final_input.partial_input.secrets {
                secrets
            } else {
                let (asset, value) = final_input
                    .partial_input
                    .asset_and_amount()
                    .ok_or(FinalTransactionError::MissingInputSecrets { input_index: i })?;

                TxOutSecrets {
                    asset,
                    asset_bf: AssetBlindingFactor::zero(),
                    value,
                    value_bf: ValueBlindingFactor::zero(),
                }
            };

            input_secrets.insert(i, secrets);
            pst.add_input(final_input.to_input());
        }

        self.outputs.iter().for_each(|el| {
            pst.add_output(el.to_output());
        });

        Ok((pst, input_secrets))
    }

    /// If the transaction or any of its inputs sets a non-zero locktime.
    fn has_locktime(&self) -> bool {
        self.locktime != LockTime::ZERO
            || self.inputs.iter().any(|input| {
                input
                    .partial_input
                    .locktime
                    .is_some_and(|locktime| locktime != LockTime::ZERO)
            })
    }
}

#[cfg(test)]
mod tests {
    use simplicityhl::elements::{AssetEntropy, LockTime, OutPoint, Script, TxOut, Txid};

    use crate::transaction::UTXO;

    use super::*;

    fn dummy_asset_id(byte: u8) -> AssetId {
        AssetId::from_byte_array([byte; 32])
    }

    fn dummy_txid(byte: u8) -> Txid {
        Txid::from_byte_array([byte; 32])
    }

    fn dummy_blinding_key() -> elements_miniscript::bitcoin::PublicKey {
        let secp = simplicityhl::elements::secp256k1_zkp::Secp256k1::new();
        let secret = simplicityhl::elements::secp256k1_zkp::SecretKey::from_slice(&[0x11; 32]).unwrap();

        elements_miniscript::bitcoin::PublicKey::new(secret.public_key(&secp))
    }

    fn explicit_utxo(txid_byte: u8, vout: u32, amount: u64, asset: AssetId) -> UTXO {
        UTXO::explicit(OutPoint::new(dummy_txid(txid_byte), vout), Script::new(), amount, asset)
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

    #[test]
    fn explicit_input_is_not_a_confidential_one() {
        let policy = dummy_asset_id(0xAA);
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(explicit_utxo(0x01, 0, 5000, policy)), None);

        assert!(!ft.has_confidential_input());
    }

    #[test]
    fn input_carrying_secrets_is_a_confidential_one() {
        let policy = dummy_asset_id(0xAA);
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(confidential_utxo(0x01, 0, policy, 5000)), None);

        assert!(ft.has_confidential_input());
    }

    #[test]
    fn confidential_input_paying_an_explicit_output_leaves_nothing_blinded() {
        let policy = dummy_asset_id(0xAA);
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(confidential_utxo(0x01, 0, policy, 5000)), None);
        ft.add_output(PartialOutput::new(Script::new(), 4000, policy));

        assert!(ft.has_confidential_input());
        assert!(!ft.needs_blinding());
    }

    #[test]
    fn blinded_output_is_what_balances_the_transaction() {
        let policy = dummy_asset_id(0xAA);
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(confidential_utxo(0x01, 0, policy, 5000)), None);
        ft.add_output(PartialOutput::new(Script::new(), 4000, policy).with_blinding_key(dummy_blinding_key()));

        assert!(ft.has_confidential_input());
        assert!(ft.needs_blinding());
    }

    // Manually construct PST and check extract_pst correctness based on it
    #[test]
    fn extract_pst_single_explicit_input_single_output() {
        let policy = dummy_asset_id(0xAA);

        let utxo = explicit_utxo(0x01, 0, 5000, policy);
        let partial_input = PartialInput::new(utxo);
        let partial_output = PartialOutput::new(Script::new(), 4000, policy);

        let mut ft = FinalTransaction::new();
        ft.add_input(partial_input.clone(), None);
        ft.add_output(partial_output.clone());

        let mut expected_pst = PartiallySignedTransaction::new_v2();
        expected_pst.add_input(partial_input.to_input());
        expected_pst.add_output(partial_output.to_output());

        let expected_secrets: HashMap<usize, TxOutSecrets> = HashMap::from([(
            0,
            TxOutSecrets::new(policy, AssetBlindingFactor::zero(), 5000, ValueBlindingFactor::zero()),
        )]);

        let (pst, secrets) = ft.extract_pst().unwrap();

        assert_eq!(pst, expected_pst);
        assert_eq!(secrets, expected_secrets);
    }

    #[test]
    fn declared_height_becomes_the_transactions_locktime() {
        let policy = dummy_asset_id(0xAA);
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(explicit_utxo(0x01, 0, 5000, policy)), None);
        ft.add_input(PartialInput::new(explicit_utxo(0x02, 0, 5000, policy)), None);
        ft.add_output(PartialOutput::new(Script::new(), 9000, policy));
        ft.set_locktime(LockTime::from_height(2_580_990).unwrap());

        let (pst, _) = ft.extract_pst().unwrap();

        assert_eq!(
            pst.locktime().expect("one height, so no conflict"),
            LockTime::from_height(2_580_990).unwrap()
        );
        assert!(pst.inputs().iter().all(|input| {
            input.required_height_locktime.is_some() && input.sequence == Some(Sequence::ENABLE_LOCKTIME_NO_RBF)
        }));
    }

    #[test]
    fn explicit_final_sequence_is_kept_under_a_locktime() {
        let policy = dummy_asset_id(0xAA);
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(explicit_utxo(0x01, 0, 5000, policy)), None);
        ft.add_output(PartialOutput::new(Script::new(), 5000, policy));
        ft.set_locktime(LockTime::from_height(100).unwrap());
        ft.set_sequence(Sequence::MAX);

        let (pst, _) = ft.extract_pst().unwrap();

        assert_eq!(pst.inputs()[0].sequence, Some(Sequence::MAX));
    }

    #[test]
    fn transaction_that_declares_no_height_still_has_none() {
        let policy = dummy_asset_id(0xAA);
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(explicit_utxo(0x01, 0, 5000, policy)), None);
        ft.add_output(PartialOutput::new(Script::new(), 4000, policy));

        let (pst, _) = ft.extract_pst().unwrap();

        assert_eq!(pst.locktime().unwrap(), LockTime::ZERO);
        assert_eq!(pst.inputs()[0].sequence, Some(Sequence::MAX));
    }

    #[test]
    fn extract_pst_single_confidential_input() {
        let policy = dummy_asset_id(0xAA);

        let utxo = confidential_utxo(0x01, 0, policy, 3000);
        let partial_input = PartialInput::new(utxo);
        let partial_output = PartialOutput::new(Script::new(), 2000, policy);

        let mut ft = FinalTransaction::new();
        ft.add_input(partial_input.clone(), None);
        ft.add_output(partial_output.clone());

        let mut expected_pst = PartiallySignedTransaction::new_v2();
        expected_pst.add_input(partial_input.to_input());
        expected_pst.add_output(partial_output.to_output());

        let expected_secrets = HashMap::from([(
            0,
            TxOutSecrets::new(policy, AssetBlindingFactor::zero(), 3000, ValueBlindingFactor::zero()),
        )]);

        let (pst, secrets) = ft.extract_pst().unwrap();

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
        ft.add_input(explicit_partial.clone(), None);
        ft.add_input(conf_partial.clone(), None);
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

        let (pst, secrets) = ft.extract_pst().unwrap();

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
        ft.add_issuance_input(partial_input.clone(), issuance.clone(), None);
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

        let (pst, secrets) = ft.extract_pst().unwrap();

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
        ft.add_issuance_input(partial_input.clone(), reissuance_input.clone(), None);
        ft.add_output(partial_output.clone());

        // build expected pst, merge partial_input and issuance manually
        let mut expected_pst = PartiallySignedTransaction::new_v2();
        let mut expected_input = partial_input.to_input();
        let issuance_input = reissuance_input.to_input();
        expected_input.issuance_value_amount = issuance_input.issuance_value_amount;
        expected_input.issuance_asset_entropy = issuance_input.issuance_asset_entropy;
        expected_input.issuance_inflation_keys = None;
        expected_input.issuance_blinding_nonce = Some(AssetBlindingNonce::from_blinding_factor(
            partial_input.secrets.unwrap().asset_bf,
        ));
        expected_input.blinded_issuance = issuance_input.blinded_issuance;
        expected_pst.add_input(expected_input);
        expected_pst.add_output(partial_output.to_output());

        let expected_secrets = HashMap::from([(
            0,
            TxOutSecrets::new(policy, AssetBlindingFactor::zero(), 1000, ValueBlindingFactor::zero()),
        )]);

        let (pst, secrets) = ft.extract_pst().unwrap();

        assert_eq!(pst, expected_pst);
        assert_eq!(secrets, expected_secrets);
    }

    #[test]
    fn balanced_transfer_single_asset() {
        let policy = dummy_asset_id(0xAA);

        let mut ft = FinalTransaction::new();
        ft.add_input(PartialInput::new(explicit_utxo(0x01, 0, 5000, policy)), None);
        ft.add_output(PartialOutput::new(Script::new(), 4000, policy));
        ft.add_output(PartialOutput::new(Script::new(), 1000, policy));

        assert!(ft.is_balanced().unwrap());
    }

    #[test]
    fn output_without_input_is_unbalanced() {
        let policy = dummy_asset_id(0xAA);

        let mut ft = FinalTransaction::new();
        ft.add_input(PartialInput::new(explicit_utxo(0x01, 0, 5000, policy)), None);
        ft.add_output(PartialOutput::new(Script::new(), 5000, policy));
        ft.add_output(PartialOutput::new(Script::new(), 1, dummy_asset_id(0xBB)));

        assert!(!ft.is_balanced().unwrap());
    }

    #[test]
    fn leftover_input_amount_is_unbalanced() {
        let policy = dummy_asset_id(0xAA);

        let mut ft = FinalTransaction::new();
        ft.add_input(PartialInput::new(explicit_utxo(0x01, 0, 5000, policy)), None);
        ft.add_output(PartialOutput::new(Script::new(), 4000, policy));

        assert!(!ft.is_balanced().unwrap());
    }

    #[test]
    fn issuance_is_balanced_against_its_declared_amounts() {
        let policy = dummy_asset_id(0xAA);

        let mut ft = FinalTransaction::new();
        let details = ft.add_issuance_input(
            PartialInput::new(explicit_utxo(0x01, 0, 1000, policy)),
            IssuanceInput::new_issuance(100, 1, [0x07; 32]),
            None,
        );
        ft.add_output(PartialOutput::new(Script::new(), 100, details.asset_id));
        ft.add_output(PartialOutput::new(Script::new(), 1, details.inflation_asset_id));
        ft.add_output(PartialOutput::new(Script::new(), 1000, policy));

        assert!(ft.is_balanced().unwrap());
    }

    #[test]
    fn issuing_more_than_declared_is_unbalanced() {
        let policy = dummy_asset_id(0xAA);

        let mut ft = FinalTransaction::new();
        let details = ft.add_issuance_input(
            PartialInput::new(explicit_utxo(0x01, 0, 1000, policy)),
            IssuanceInput::new_issuance(100, 0, [0x07; 32]),
            None,
        );
        ft.add_output(PartialOutput::new(Script::new(), 1_000_000_000, details.asset_id));
        ft.add_output(PartialOutput::new(Script::new(), 1000, policy));

        assert!(!ft.is_balanced().unwrap());
    }

    #[test]
    fn minting_inflation_keys_that_were_never_declared_is_unbalanced() {
        let policy = dummy_asset_id(0xAA);

        let mut ft = FinalTransaction::new();
        let details = ft.add_issuance_input(
            PartialInput::new(explicit_utxo(0x01, 0, 1000, policy)),
            IssuanceInput::new_issuance(100, 0, [0x07; 32]),
            None,
        );
        ft.add_output(PartialOutput::new(Script::new(), 100, details.asset_id));
        ft.add_output(PartialOutput::new(Script::new(), 42, details.inflation_asset_id));
        ft.add_output(PartialOutput::new(Script::new(), 1000, policy));

        assert!(!ft.is_balanced().unwrap());
    }

    #[test]
    fn reissuance_alongside_a_transfer_of_the_same_asset_is_balanced() {
        let policy = dummy_asset_id(0xAA);
        let entropy = [0x07; 32];

        let mut probe = FinalTransaction::new();
        let details = probe.add_issuance_input(
            PartialInput::new(explicit_utxo(0x01, 0, 1, dummy_asset_id(0xBB))),
            IssuanceInput::new_reissuance(100, entropy),
            None,
        );

        let mut ft = FinalTransaction::new();
        ft.add_input(
            PartialInput::new(explicit_utxo(0x02, 0, 1, details.inflation_asset_id)),
            None,
        );
        ft.add_issuance_input(
            PartialInput::new(explicit_utxo(0x03, 0, 50, details.asset_id)),
            IssuanceInput::new_reissuance(100, entropy),
            None,
        );
        ft.add_input(PartialInput::new(explicit_utxo(0x04, 0, 1000, policy)), None);

        ft.add_output(PartialOutput::new(Script::new(), 150, details.asset_id));
        ft.add_output(PartialOutput::new(Script::new(), 1, details.inflation_asset_id));
        ft.add_output(PartialOutput::new(Script::new(), 1000, policy));

        assert!(ft.is_balanced().unwrap());
    }

    #[test]
    fn confidential_input_amounts_are_counted() {
        let policy = dummy_asset_id(0xAA);

        let mut ft = FinalTransaction::new();
        ft.add_input(PartialInput::new(confidential_utxo(0x01, 0, policy, 5000)), None);
        ft.add_output(PartialOutput::new(Script::new(), 5000, policy));

        assert!(ft.is_balanced().unwrap());
    }

    #[test]
    fn balanced_transfer_single_asset_with_metadata_output() {
        let policy = dummy_asset_id(0xAA);

        let mut ft = FinalTransaction::new();
        ft.add_input(PartialInput::new(explicit_utxo(0x01, 0, 5000, policy)), None);
        ft.add_output(PartialOutput::new(Script::new(), 4000, policy));
        ft.add_output(PartialOutput::new(Script::new(), 1000, policy));
        ft.add_output(PartialOutput::new_metadata("burn".as_bytes()));

        assert!(ft.is_balanced().unwrap());
    }

    #[test]
    #[should_panic(expected = "Reissuance input must be confidential")]
    fn reissuance_without_secrets_panics_during_extraction() {
        let entropy = [0x42; 32];
        let token = AssetId::reissuance_token_from_entropy(AssetEntropy::from_byte_array(entropy), false);
        let input = PartialInput::new(explicit_utxo(1, 0, 1, token));

        let mut tx = FinalTransaction::new();
        tx.add_issuance_input(input, IssuanceInput::new_reissuance(5_000, entropy), None);
        assert_eq!(tx.inputs()[0].get_asset_and_amount(), Some((token, 1)));

        // Reproduces the documented panic in FinalInput::to_input.
        let _ = tx.extract_pst();
    }

    #[test]
    fn reissuance_with_secrets_preserves_the_blinding_nonce() {
        use simplicityhl::elements::confidential::{Asset, Value};

        let entropy = [0x12; 32];
        let token = AssetId::reissuance_token_from_entropy(AssetEntropy::from_byte_array(entropy), false);

        // A nonzero asset blinder distinguishes reissuance from a new issuance.
        let asset_bf = AssetBlindingFactor::from_slice(&[1; 32]).unwrap();
        let value_bf = ValueBlindingFactor::from_slice(&[2; 32]).unwrap();
        let expected_secrets = TxOutSecrets::new(token, asset_bf, 1, value_bf);

        let secp = simplicityhl::elements::secp256k1_zkp::Secp256k1::new();

        let input = PartialInput::new(UTXO {
            outpoint: OutPoint::new(dummy_txid(1), 0),
            txout: TxOut {
                asset: Asset::new_confidential(&secp, token, asset_bf),
                value: Value::new_confidential_from_assetid(&secp, 1, token, value_bf, asset_bf),
                ..Default::default()
            },
            secrets: Some(expected_secrets),
        });

        let mut tx = FinalTransaction::new();
        tx.add_issuance_input(input, IssuanceInput::new_reissuance(5_000, entropy), None);

        let (pst, secrets) = tx.extract_pst().unwrap();

        assert_eq!(secrets.get(&0), Some(&expected_secrets));
        assert_eq!(pst.inputs()[0].issuance_value_amount, Some(5_000));
        assert_eq!(
            pst.inputs()[0].issuance_blinding_nonce,
            Some(AssetBlindingNonce::from_blinding_factor(asset_bf))
        );
    }
}
