use std::collections::{HashMap, HashSet};

use bip39::rand::thread_rng;

use simplicityhl::elements::bitcoin::PublicKey;
use simplicityhl::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use simplicityhl::elements::pset::{Input, PartiallySignedTransaction, PsetBlindError};
use simplicityhl::elements::secp256k1_zkp::{SECP256K1, SecretKey};
use simplicityhl::elements::{
    AssetBlindingNonce, AssetId, LockTime, RangeProof, Sequence, SurjectionInput, SurjectionProof, TxOut, TxOutSecrets,
};

use crate::provider::SimplicityNetwork;

use super::change_output::ChangeOutput;
use super::error::{FinalTransactionError, PartialInputError};
use super::partial_input::{IssuanceDetails, IssuanceInput, PartialInput, ProgramInput};
use super::partial_output::PartialOutput;
use super::required_signature::{NativeSignature, ProgramSignature, RequiredSignature};

/// Weight units per virtual byte.
const WITNESS_SCALE_FACTOR: usize = 4;

/// One input of a [`FinalTransaction`]: the output it spends, and optionally the program that
/// locks that output and an asset issuance, together with the signature the signer adds.
///
/// [`FinalTransaction::inputs`] returns these. All fields are public, so they can be changed
/// through [`FinalTransaction::inputs_mut`].
#[derive(Clone, Debug)]
pub struct FinalInput {
    /// The output this input spends.
    pub partial_input: PartialInput,
    /// The program that locks the spent output and its witness, for a program input.
    pub program_input: Option<ProgramInput>,
    /// The asset this input issues or reissues, if any.
    pub issuance_input: Option<IssuanceInput>,
    /// The signature the signer adds to this input.
    pub required_sig: RequiredSignature,
}

impl FinalInput {
    /// Creates an input without a program or an issuance.
    #[must_use]
    pub fn new(partial_input: PartialInput, required_sig: RequiredSignature) -> Self {
        Self {
            partial_input,
            required_sig,
            program_input: None,
            issuance_input: None,
        }
    }

    /// Sets the program that locks the spent output.
    #[must_use]
    pub fn with_program(mut self, program_input: ProgramInput) -> Self {
        self.program_input = Some(program_input);

        self
    }

    /// Sets the asset this input issues or reissues.
    #[must_use]
    pub fn with_issuance(mut self, issuance_input: IssuanceInput) -> Self {
        self.issuance_input = Some(issuance_input);

        self
    }

    /// The asset IDs and entropy of this input's issuance or reissuance, or `None` if it doesn't
    /// issue anything.
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

    /// Converts this input, with its issuance, into a PSET input.
    ///
    /// The input's own sequence and locktime are used as they are. [`FinalTransaction::extract_pst`]
    /// fills in the transaction-wide ones first and checks the issuance, so prefer `extract_pst`.
    ///
    /// # Panics
    /// Panics if the input reissues an asset and `partial_input.secrets` is `None`, because the
    /// reissuance needs the asset blinding factor of the token it spends.
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

    fn check_issuance(&self, input_index: usize) -> Result<(), FinalTransactionError> {
        match &self.issuance_input {
            Some(
                IssuanceInput::Issuance {
                    issuance_amount: 0,
                    inflation_amount: 0,
                    ..
                }
                | IssuanceInput::Reissuance { issuance_amount: 0, .. },
            ) => Err(FinalTransactionError::EmptyIssuance { input_index }),
            Some(IssuanceInput::Reissuance { .. })
                if self
                    .partial_input
                    .secrets
                    .is_none_or(|secrets| secrets.asset_bf == AssetBlindingFactor::zero()) =>
            {
                Err(FinalTransactionError::UnblindedReissuanceToken { input_index })
            }
            _ => Ok(()),
        }
    }
}

/// A transaction under construction: inputs, outputs, an optional change target, and the
/// transaction-wide sequence and locktime.
///
/// Build one with [`FinalTransaction::new`] and the `add_*` methods, then hand it to a
/// [`Signer`](crate::signer::Signer). The signer can add wallet inputs to pay the fee, appends a
/// fee output and, unless the change is too small to keep, a change output before it, then
/// blinds, signs, and runs every program input. Inputs and outputs keep the order in which they
/// were added, and anything the signer adds comes after them.
///
/// The policy-asset amounts are what pay the fee. Every other asset must already balance: add an
/// output for every non-policy asset the inputs bring in.
#[derive(Clone, Debug)]
pub struct FinalTransaction {
    inputs: Vec<FinalInput>,
    outputs: Vec<PartialOutput>,
    change: Option<ChangeOutput>,
    sequence: Option<Sequence>,
    locktime: LockTime,
}

impl FinalTransaction {
    /// Creates an empty transaction with no change target, no transaction-wide sequence, and a
    /// zero locktime.
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

    /// Sets the sequence of every input that doesn't set its own with
    /// [`PartialInput::with_sequence`].
    pub fn set_sequence(&mut self, sequence: Sequence) {
        self.sequence = Some(sequence);
    }

    /// Sets the locktime of every input that doesn't set its own with
    /// [`PartialInput::with_locktime`].
    ///
    /// A non-zero locktime also makes [`FinalTransaction::sequence`] default to
    /// `Sequence::ENABLE_LOCKTIME_NO_RBF`, because nodes ignore the locktime of a transaction whose
    /// inputs all have the final sequence.
    pub fn set_locktime(&mut self, locktime: LockTime) {
        self.locktime = locktime;
    }

    /// The sequence of the inputs that don't set their own.
    ///
    /// This is the value given to [`FinalTransaction::set_sequence`]. Without one, it is
    /// `Sequence::ENABLE_LOCKTIME_NO_RBF` if the transaction or any input sets a non-zero locktime,
    /// so that the locktime is enforced, and `Sequence::MAX` otherwise.
    #[must_use]
    pub fn sequence(&self) -> Sequence {
        self.sequence.unwrap_or(if self.has_locktime() {
            Sequence::ENABLE_LOCKTIME_NO_RBF
        } else {
            Sequence::MAX
        })
    }

    /// The locktime of the inputs that don't set their own, as given to
    /// [`FinalTransaction::set_locktime`].
    #[must_use]
    pub fn locktime(&self) -> LockTime {
        self.locktime
    }

    /// Sets where the signer sends this transaction's change.
    ///
    /// Without a change target, change goes to the signer's own address, blinded with the
    /// signer's blinding key. An explicit target works only if the transaction has another
    /// blinded output or doesn't spend any confidential input.
    pub fn add_change(&mut self, change: ChangeOutput) {
        self.change = Some(change);
    }

    /// Drops the change target, returning to the signer's own address.
    pub fn remove_change(&mut self) {
        self.change = None;
    }

    /// Adds an input that spends an output not locked by a Simplicity program, such as a wallet
    /// UTXO.
    ///
    /// With `Some(NativeSignature::Ecdsa)`, the signer signs it as a P2WPKH input with the key at
    /// the input's derivation path. With `None`, the signer leaves it unsigned, for its owner to
    /// sign.
    pub fn add_input(&mut self, partial_input: PartialInput, signature: Option<NativeSignature>) {
        self.inputs.push(FinalInput::new(partial_input, signature.into()));
    }

    /// Adds an input that spends an output locked by a Simplicity program.
    ///
    /// With `Some(signature)`, the signer signs the input and puts the signature into the witness
    /// field that `signature` names. It replaces the field's value supplied in `program_input`,
    /// or only the value at the end of the path given to [`ProgramSignature::at_path`]. With
    /// `None`, the witness is used as supplied.
    pub fn add_program_input(
        &mut self,
        partial_input: PartialInput,
        program_input: ProgramInput,
        signature: Option<ProgramSignature>,
    ) {
        self.inputs
            .push(FinalInput::new(partial_input, signature.into()).with_program(program_input));
    }

    /// Adds a native input, as [`FinalTransaction::add_input`] does, that also issues or reissues
    /// an asset.
    ///
    /// Returns the IDs of the issued asset and its reissuance token, and the asset entropy that a
    /// later reissuance needs.
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

    /// Adds a program input, as [`FinalTransaction::add_program_input`] does, that also issues or
    /// reissues an asset.
    ///
    /// Returns the IDs of the issued asset and its reissuance token, and the asset entropy that a
    /// later reissuance needs.
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

    /// Appends an output.
    pub fn add_output(&mut self, partial_output: PartialOutput) {
        self.outputs.push(partial_output);
    }

    /// Removes the output at `index`, `None` when there is none.
    pub fn remove_output(&mut self, index: usize) -> Option<PartialOutput> {
        (index < self.outputs.len()).then(|| self.outputs.remove(index))
    }

    /// The change target set with [`FinalTransaction::add_change`], if any.
    #[must_use]
    pub fn change(&self) -> Option<&ChangeOutput> {
        self.change.as_ref()
    }

    /// The change target set with [`FinalTransaction::add_change`], if any, for changing the target
    /// in place.
    #[must_use]
    pub fn change_mut(&mut self) -> Option<&mut ChangeOutput> {
        self.change.as_mut()
    }

    /// The inputs, in the order they were added.
    #[must_use]
    pub fn inputs(&self) -> &[FinalInput] {
        &self.inputs
    }

    /// The inputs, for changing them in place.
    pub fn inputs_mut(&mut self) -> &mut [FinalInput] {
        &mut self.inputs
    }

    /// The outputs, in the order they were added.
    #[must_use]
    pub fn outputs(&self) -> &[PartialOutput] {
        &self.outputs
    }

    /// The outputs, for changing them in place.
    pub fn outputs_mut(&mut self) -> &mut [PartialOutput] {
        &mut self.outputs
    }

    /// The number of inputs.
    #[must_use]
    pub fn n_inputs(&self) -> usize {
        self.inputs.len()
    }

    /// The number of outputs.
    #[must_use]
    pub fn n_outputs(&self) -> usize {
        self.outputs.len()
    }

    /// Whether any output has a blinding key, so that the signer blinds the transaction.
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

    /// The amount of `network`'s policy asset that the inputs bring in, minus the amount the
    /// outputs pay. This is what remains for the fee and change.
    ///
    /// Only the outputs the inputs spend count. Amounts an input issues don't.
    ///
    /// # Errors
    /// Returns [`FinalTransactionError::MissingInputSecrets`] if an input's asset or
    /// amount is unknown and no unblinding secrets are available.
    /// Returns [`FinalTransactionError::AmountOverflow`] if the delta doesn't fit `i64`.
    pub fn calculate_fee_delta(&self, network: &SimplicityNetwork) -> Result<i64, FinalTransactionError> {
        let mut available_amount: i128 = 0;

        for (input_index, input) in self.inputs.iter().enumerate() {
            let (asset, amount) = input
                .get_asset_and_amount()
                .ok_or(FinalTransactionError::MissingInputSecrets { input_index })?;
            if asset == network.policy_asset() {
                available_amount += i128::from(amount);
            }
        }

        let consumed_amount: i128 = self
            .outputs
            .iter()
            .filter(|output| output.asset == network.policy_asset())
            .map(|output| i128::from(output.amount))
            .sum();

        i64::try_from(available_amount - consumed_amount).map_err(|_| FinalTransactionError::AmountOverflow)
    }

    /// Whether, for every asset, the inputs bring in exactly what the outputs pay.
    ///
    /// Issued and reissued amounts count as inputs, so a newly created asset is balanced against
    /// its declared `issuance_amount` and `inflation_amount`. Zero-value `OP_RETURN` outputs are
    /// skipped. An output paying an asset that no input brings in makes the transaction
    /// unbalanced.
    ///
    /// The fee is an output, so a transaction is balanced only once its fee output is present.
    ///
    /// # Errors
    /// Returns [`FinalTransactionError::MissingInputSecrets`] if an input's asset or
    /// amount is unknown and no unblinding secrets are available.
    pub fn is_balanced(&self) -> Result<bool, FinalTransactionError> {
        let mut transfers: HashMap<AssetId, i128> = HashMap::new();

        // Collecting all inputs
        for (input_index, input) in self.inputs.iter().enumerate() {
            let (asset, amount) = input
                .get_asset_and_amount()
                .ok_or(FinalTransactionError::MissingInputSecrets { input_index })?;

            let transfer_entry = transfers.entry(asset).or_insert(0);
            *transfer_entry += i128::from(amount);

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

                *transfers.entry(issuance_details.asset_id).or_insert(0) += i128::from(issuance_amount);
                *transfers.entry(issuance_details.inflation_asset_id).or_insert(0) += i128::from(inflation_amount);
            }
        }

        for output in &self.outputs {
            if output.script_pubkey.is_op_return() && output.amount == 0 {
                continue;
            }

            match transfers.get_mut(&output.asset) {
                Some(value) => {
                    *value -= i128::from(output.amount);
                }
                None => return Ok(false),
            }
        }

        // All transfers including fee should sum up to 0
        Ok(transfers.values().all(|&value| value == 0))
    }

    /// Checks every input with [`PartialInput::check_asset_and_amount`].
    ///
    /// # Errors
    /// Returns the error of the first input that fails the check.
    pub fn check_inputs_assets_and_amounts(&self) -> Result<(), PartialInputError> {
        self.inputs
            .iter()
            .try_for_each(|input| input.partial_input.check_asset_and_amount())
    }

    /// Checks that no two inputs spend the same outpoint.
    ///
    /// # Errors
    /// Returns [`FinalTransactionError::DuplicateInput`] with the first outpoint spent twice.
    pub fn check_unique_inputs(&self) -> Result<(), FinalTransactionError> {
        let mut seen = HashSet::new();

        match self
            .inputs
            .iter()
            .find(|input| !seen.insert(input.partial_input.outpoint))
        {
            Some(input) => Err(FinalTransactionError::DuplicateInput(input.partial_input.outpoint)),
            None => Ok(()),
        }
    }

    /// Checks that the signer can blind the outputs with the factors they choose.
    ///
    /// # Errors
    /// - [`FinalTransactionError::ChosenFactorOnExplicitOutput`] if an output without a blinding
    ///   key chooses a factor.
    /// - [`FinalTransactionError::SurjectionWithoutShift`] if an output chooses the asset
    ///   blinding factor an input or an issuance brings the same asset in with.
    /// - [`FinalTransactionError::NoFreeValueFactor`] if every blinded output chooses its value
    ///   blinding factor.
    pub fn check_blinding_factors(&self) -> Result<(), FinalTransactionError> {
        for (output_index, output) in self.outputs.iter().enumerate() {
            let chooses = output.asset_blinding_factor.is_some() || output.value_blinding_factor.is_some();

            if chooses && output.blinding_key.is_none() {
                return Err(FinalTransactionError::ChosenFactorOnExplicitOutput { output_index });
            }

            if output
                .asset_blinding_factor
                .is_some_and(|asset_bf| self.has_input_with_asset_blinding_factor(output.asset, asset_bf))
            {
                return Err(FinalTransactionError::SurjectionWithoutShift { output_index });
            }
        }

        let free = self
            .outputs
            .iter()
            .any(|output| output.blinding_key.is_some() && output.value_blinding_factor.is_none());

        if self.needs_blinding() && !free {
            return Err(FinalTransactionError::NoFreeValueFactor);
        }

        Ok(())
    }

    /// Converts the transaction into an unsigned PSET, and returns it with each input's secrets,
    /// by input index.
    ///
    /// Each input without its own sequence or locktime gets [`FinalTransaction::sequence`] and
    /// [`FinalTransaction::locktime`]. An explicit input gets secrets with zero blinding
    /// factors, so that blinding can balance against the explicit amount. Outputs are converted
    /// as they are: this doesn't add any change or fee output.
    ///
    /// This is where code that signs outside the [`Signer`](crate::signer::Signer) takes over.
    ///
    /// # Errors
    /// Returns [`FinalTransactionError::MissingInputSecrets`] if an input's asset or
    /// amount is unknown and no unblinding secrets are available.
    ///
    /// Returns [`FinalTransactionError::EmptyIssuance`] if an issuance input doesn't issue anything, and
    /// [`FinalTransactionError::UnblindedReissuanceToken`] if a reissuance spends a token that isn't blinded.
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

            final_input.check_issuance(i)?;

            input_secrets.insert(i, secrets);
            pst.add_input(final_input.to_input());
        }

        self.outputs.iter().for_each(|el| {
            pst.add_output(el.to_output());
        });

        Ok((pst, input_secrets))
    }

    /// Converts the transaction into a PSET, as [`FinalTransaction::extract_pst`] does, and blinds
    /// every output that has a blinding key.
    ///
    /// Each blinded output uses the blinding factors it chooses and random ones for the rest. The last
    /// blinded output that doesn't choose its value blinding factor gets the one that balances
    /// the transaction.
    ///
    /// # Errors
    /// Returns the errors of [`FinalTransaction::extract_pst`] and
    /// [`FinalTransaction::check_blinding_factors`], and [`FinalTransactionError::Blinding`] if a
    /// commitment or a proof can't be made.
    pub fn extract_blinded_pst(&self) -> Result<PartiallySignedTransaction, FinalTransactionError> {
        self.check_blinding_factors()?;

        let (mut pst, input_secrets) = self.extract_pst()?;

        // The check leaves a free value factor whenever an output is blinded
        let Some(balancer) = self
            .outputs
            .iter()
            .rposition(|output| output.blinding_key.is_some() && output.value_blinding_factor.is_none())
        else {
            return Ok(pst);
        };

        let surjection_inputs = pst.surjection_inputs(&input_secrets)?;
        let mut output_factors = Vec::new();

        for (index, output) in self.outputs.iter().enumerate() {
            if index == balancer {
                continue;
            }

            if output.blinding_key.is_none() {
                output_factors.push((output.amount, AssetBlindingFactor::zero(), ValueBlindingFactor::zero()));

                continue;
            }

            let asset_bf = output
                .asset_blinding_factor
                .unwrap_or_else(|| AssetBlindingFactor::new(&mut thread_rng()));
            let value_bf = output
                .value_blinding_factor
                .unwrap_or_else(|| ValueBlindingFactor::new(&mut thread_rng()));

            self.blind_output(&mut pst, index, asset_bf, value_bf, &surjection_inputs)?;
            output_factors.push((output.amount, asset_bf, value_bf));
        }

        let input_factors: Vec<_> = input_secrets
            .values()
            .map(|secrets| (secrets.value, secrets.asset_bf, secrets.value_bf))
            .collect();
        let asset_bf = self.outputs[balancer]
            .asset_blinding_factor
            .unwrap_or_else(|| AssetBlindingFactor::new(&mut thread_rng()));
        let value_bf = ValueBlindingFactor::last(
            SECP256K1,
            self.outputs[balancer].amount,
            asset_bf,
            &input_factors,
            &output_factors,
        );

        self.blind_output(&mut pst, balancer, asset_bf, value_bf, &surjection_inputs)?;

        Ok(pst)
    }

    /// The fee in satoshis for `weight` at `fee_rate` sats/kvb, both rounded up.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    pub(crate) fn calculate_fee(weight: usize, fee_rate: f32) -> u64 {
        let vsize = weight.div_ceil(WITNESS_SCALE_FACTOR);

        (vsize as f32 * fee_rate / 1000.0).ceil() as u64
    }

    /// Blinds output `index` of `pst` with `asset_bf` and `value_bf`, using this transaction's
    /// output `index` for the asset, amount, script and blinding key.
    ///
    /// Sets the output's commitments, range and surjection proofs, and ECDH nonce, as
    /// `PartiallySignedTransaction::blind_last` does.
    fn blind_output(
        &self,
        pst: &mut PartiallySignedTransaction,
        index: usize,
        asset_bf: AssetBlindingFactor,
        value_bf: ValueBlindingFactor,
        surjection_inputs: &[SurjectionInput],
    ) -> Result<(), PsetBlindError> {
        let rng = &mut thread_rng();
        let output = &self.outputs[index];
        let blinding_key = output.blinding_key.expect("a blinded output has a blinding key");
        let ephemeral_key = SecretKey::new(rng);

        let blinded = TxOut::with_txout_secrets(
            rng,
            SECP256K1,
            output.script_pubkey.clone(),
            blinding_key.inner,
            ephemeral_key,
            TxOutSecrets::new(output.asset, asset_bf, output.amount, value_bf),
            surjection_inputs,
        )
        .map_err(|e| PsetBlindError::ConfidentialTxOutError(index, e))?;

        let asset_generator = blinded.asset.commitment().expect("a blinded asset");
        let value_commitment = blinded.value.commitment().expect("a blinded value");
        let pst_output = &mut pst.outputs_mut()[index];

        pst_output.asset_comm = Some(asset_generator);
        pst_output.amount_comm = Some(value_commitment);
        pst_output.asset_surjection_proof = Some(blinded.witness.surjection_proof);
        pst_output.value_rangeproof = Some(blinded.witness.rangeproof);
        pst_output.ecdh_pubkey = blinded.nonce.commitment().map(|inner| PublicKey {
            inner,
            compressed: true,
        });
        pst_output.blind_asset_proof = Some(
            SurjectionProof::blind_asset_proof(rng, SECP256K1, output.asset, asset_bf)
                .map_err(|e| PsetBlindError::BlindingProofsCreationError(index, e))?,
        );
        pst_output.blind_value_proof = Some(
            RangeProof::blind_value_proof(
                rng,
                SECP256K1,
                output.amount,
                value_commitment,
                asset_generator,
                value_bf,
            )
            .map_err(|e| PsetBlindError::BlindingProofsCreationError(index, e))?,
        );

        Ok(())
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

    /// If an input or an issuance carries `asset` with the asset blinding factor `asset_bf`.
    /// Explicit inputs and issuances carry their assets with a zero factor.
    fn has_input_with_asset_blinding_factor(&self, asset: AssetId, asset_bf: AssetBlindingFactor) -> bool {
        self.inputs.iter().any(|input| {
            let spent = input.partial_input.asset_and_amount().map(|(spent, _)| spent);
            let spent_bf = input
                .partial_input
                .secrets
                .map_or(AssetBlindingFactor::zero(), |secrets| secrets.asset_bf);
            let issues = input
                .get_issuance_details()
                .is_some_and(|details| details.asset_id == asset || details.inflation_asset_id == asset);

            (spent == Some(asset) && spent_bf == asset_bf) || (issues && asset_bf == AssetBlindingFactor::zero())
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
    fn chosen_blinding_factors_are_checked() {
        let policy = dummy_asset_id(0xAA);
        let asset_bf = AssetBlindingFactor::from_slice(&[0x01; 32]).unwrap();
        let value_bf = ValueBlindingFactor::from_slice(&[0x01; 32]).unwrap();
        let blinded = || PartialOutput::new(Script::new(), 1000, policy).with_blinding_key(dummy_blinding_key());
        let check = |outputs: Vec<PartialOutput>| {
            let mut ft = FinalTransaction::new();

            ft.add_input(PartialInput::new(explicit_utxo(0x01, 0, 5000, policy)), None);
            for output in outputs {
                ft.add_output(output);
            }

            ft.check_blinding_factors()
        };

        assert_eq!(
            check(vec![
                blinded()
                    .with_asset_blinding_factor(asset_bf)
                    .with_value_blinding_factor(value_bf),
                blinded(),
            ]),
            Ok(())
        );
        assert_eq!(
            check(vec![
                PartialOutput::new(Script::new(), 1000, policy).with_value_blinding_factor(value_bf)
            ]),
            Err(FinalTransactionError::ChosenFactorOnExplicitOutput { output_index: 0 })
        );
        // The explicit input brings the asset in with a zero factor
        assert_eq!(
            check(vec![
                blinded(),
                blinded().with_asset_blinding_factor(AssetBlindingFactor::zero()),
            ]),
            Err(FinalTransactionError::SurjectionWithoutShift { output_index: 1 })
        );
        assert_eq!(
            check(vec![blinded().with_value_blinding_factor(value_bf)]),
            Err(FinalTransactionError::NoFreeValueFactor)
        );
    }

    #[test]
    fn explicit_input_is_not_confidential() {
        let policy = dummy_asset_id(0xAA);
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(explicit_utxo(0x01, 0, 5000, policy)), None);

        assert!(!ft.has_confidential_input());
    }

    #[test]
    fn input_with_secrets_is_confidential() {
        let policy = dummy_asset_id(0xAA);
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(confidential_utxo(0x01, 0, policy, 5000)), None);

        assert!(ft.has_confidential_input());
    }

    #[test]
    fn explicit_output_needs_no_blinding() {
        let policy = dummy_asset_id(0xAA);
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(confidential_utxo(0x01, 0, policy, 5000)), None);
        ft.add_output(PartialOutput::new(Script::new(), 4000, policy));

        assert!(ft.has_confidential_input());
        assert!(!ft.needs_blinding());
    }

    #[test]
    fn output_with_blinding_key_needs_blinding() {
        let policy = dummy_asset_id(0xAA);
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(confidential_utxo(0x01, 0, policy, 5000)), None);
        ft.add_output(PartialOutput::new(Script::new(), 4000, policy).with_blinding_key(dummy_blinding_key()));

        assert!(ft.has_confidential_input());
        assert!(ft.needs_blinding());
    }

    // Build the expected PST by hand and compare `extract_pst` with the result
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
    fn locktime_height_applies_to_every_input() {
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
    fn no_locktime_keeps_final_sequences() {
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

        // A reissuance token needs a non-zero asset blinding factor
        let token_secrets = TxOutSecrets::new(
            policy,
            AssetBlindingFactor::from_slice(&[1; 32]).unwrap(),
            1000,
            ValueBlindingFactor::zero(),
        );
        let mut conf_utxo = confidential_utxo(0x02, 0, policy, 1000);
        conf_utxo.secrets = Some(token_secrets);

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

        let expected_secrets = HashMap::from([(0, token_secrets)]);

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
    fn amounts_beyond_i64_do_not_wrap() {
        let policy = dummy_asset_id(0xAA);
        let network = SimplicityNetwork::ElementsRegtest { policy_asset: policy };
        let mut ft = FinalTransaction::new();

        ft.add_input(PartialInput::new(explicit_utxo(0x01, 0, 100_000, policy)), None);
        ft.add_output(PartialOutput::new(Script::new(), u64::MAX, policy));

        assert_eq!(
            ft.calculate_fee_delta(&network),
            Err(FinalTransactionError::AmountOverflow)
        );
        assert_eq!(ft.is_balanced(), Ok(false));
    }

    #[test]
    fn reissuance_with_an_explicit_token_is_rejected() {
        let entropy = [0x42; 32];
        let token = AssetId::reissuance_token_from_entropy(AssetEntropy::from_byte_array(entropy), false);

        let mut tx = FinalTransaction::new();
        tx.add_issuance_input(
            PartialInput::new(explicit_utxo(1, 0, 1, token)),
            IssuanceInput::new_reissuance(5_000, entropy),
            None,
        );

        assert!(matches!(
            tx.extract_pst(),
            Err(FinalTransactionError::UnblindedReissuanceToken { input_index: 0 })
        ));
    }

    #[test]
    fn token_only_issuance_is_allowed_and_empty_issuance_is_rejected() {
        let policy = dummy_asset_id(0xAA);
        let issuance = |issuance_amount, inflation_amount| {
            let mut tx = FinalTransaction::new();
            tx.add_issuance_input(
                PartialInput::new(explicit_utxo(1, 0, 1000, policy)),
                IssuanceInput::new_issuance(issuance_amount, inflation_amount, [0x42; 32]),
                None,
            );

            tx.extract_pst()
        };

        let (pst, _) = issuance(0, 1).unwrap();

        assert_eq!(pst.inputs()[0].issuance_value_amount, None);
        assert_eq!(pst.inputs()[0].issuance_inflation_keys, Some(1));
        assert!(matches!(
            issuance(0, 0),
            Err(FinalTransactionError::EmptyIssuance { input_index: 0 })
        ));
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
