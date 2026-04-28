use std::collections::HashMap;

use bitcoin_hashes::sha256;
use simplicityhl::elements::pset::{Input, PartiallySignedTransaction};
use simplicityhl::elements::{
    AssetId, TxOutSecrets,
    confidential::{AssetBlindingFactor, ValueBlindingFactor},
};

use crate::provider::SimplicityNetwork;
use crate::transaction::partial_input::ReissuanceInput;
use crate::utils::asset_entropy;

use super::partial_input::{IssuanceInput, PartialInput, ProgramInput, RequiredSignature};
use super::partial_output::PartialOutput;

pub const WITNESS_SCALE_FACTOR: usize = 4;

#[derive(Clone)]
pub struct FinalInput {
    pub partial_input: PartialInput,
    pub program_input: Option<ProgramInput>,
    pub issuance_input: Option<IssuanceInput>,
    pub reissuance_input: Option<ReissuanceInput>,
    pub required_sig: RequiredSignature,
}

impl FinalInput {
    pub fn new(partial_input: PartialInput, required_sig: RequiredSignature) -> Self {
        Self {
            partial_input,
            required_sig,
            program_input: None,
            issuance_input: None,
            reissuance_input: None,
        }
    }

    pub fn with_program(mut self, program_input: ProgramInput) -> Self {
        self.program_input = Some(program_input);

        self
    }

    pub fn with_issuance(mut self, issuance_input: IssuanceInput) -> Self {
        self.issuance_input = Some(issuance_input);

        self
    }

    pub fn with_reissuance(mut self, reissuance_input: ReissuanceInput) -> Self {
        self.reissuance_input = Some(reissuance_input);

        self
    }

    pub fn to_input(&self) -> Input {
        let mut pst_input = self.partial_input.to_input();

        // populate the input manually since `input.merge` is private
        if self.issuance_input.is_some() {
            let issue = self.issuance_input.clone().unwrap().to_input();

            pst_input.issuance_value_amount = issue.issuance_value_amount;
            pst_input.issuance_asset_entropy = issue.issuance_asset_entropy;
            pst_input.issuance_inflation_keys = issue.issuance_inflation_keys;
            pst_input.blinded_issuance = issue.blinded_issuance;
        } else if self.reissuance_input.is_some() {
            let reissue = self.reissuance_input.clone().unwrap().to_input();
            let issuance_blinding_nonce = self
                .partial_input
                .secrets
                .expect("Reissuance input must be confidential")
                .asset_bf
                .into_inner();

            pst_input.issuance_value_amount = reissue.issuance_value_amount;
            pst_input.issuance_asset_entropy = reissue.issuance_asset_entropy;
            pst_input.issuance_blinding_nonce = Some(issuance_blinding_nonce);
            pst_input.blinded_issuance = reissue.blinded_issuance;
        }

        pst_input
    }
}

#[derive(Clone)]
pub struct FinalTransaction {
    inputs: Vec<FinalInput>,
    outputs: Vec<PartialOutput>,
}

impl FinalTransaction {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    pub fn add_input(&mut self, partial_input: PartialInput, required_sig: RequiredSignature) {
        match required_sig {
            RequiredSignature::Witness(_) | RequiredSignature::WitnessWithPath(_, _) => {
                panic!("Requested signature is not NativeEcdsa or None")
            }
            _ => {}
        };

        self.inputs.push(FinalInput::new(partial_input, required_sig));
    }

    pub fn add_program_input(
        &mut self,
        partial_input: PartialInput,
        program_input: ProgramInput,
        required_sig: RequiredSignature,
    ) {
        if let RequiredSignature::NativeEcdsa = required_sig {
            panic!("Requested signature is not Witness or None");
        }

        self.inputs
            .push(FinalInput::new(partial_input, required_sig).with_program(program_input));
    }

    pub fn add_issuance_input(
        &mut self,
        partial_input: PartialInput,
        issuance_input: IssuanceInput,
        required_sig: RequiredSignature,
    ) -> (AssetId, AssetId) {
        match required_sig {
            RequiredSignature::Witness(_) | RequiredSignature::WitnessWithPath(_, _) => {
                panic!("Requested signature is not NativeEcdsa or None")
            }
            _ => {}
        };

        let entropy = asset_entropy(&partial_input.outpoint(), issuance_input.asset_entropy);

        let issuance_asset_id = AssetId::from_entropy(entropy);
        let reissuance_asset_id = AssetId::reissuance_token_from_entropy(entropy, false);

        self.inputs
            .push(FinalInput::new(partial_input, required_sig).with_issuance(issuance_input));

        (issuance_asset_id, reissuance_asset_id)
    }

    pub fn add_reissuance_input(
        &mut self,
        partial_input: PartialInput,
        reissuance_input: ReissuanceInput,
        required_sig: RequiredSignature,
    ) -> AssetId {
        match required_sig {
            RequiredSignature::Witness(_) | RequiredSignature::WitnessWithPath(_, _) => {
                panic!("Requested signature is not NativeEcdsa or None")
            }
            _ => {}
        };

        if partial_input.secrets.is_none() {
            panic!("Reissuance input must be confidential")
        }

        let asset_entropy = sha256::Midstate::from_byte_array(reissuance_input.asset_entropy);

        let issuance_asset_id = AssetId::from_entropy(asset_entropy);

        self.inputs
            .push(FinalInput::new(partial_input, required_sig).with_reissuance(reissuance_input));

        issuance_asset_id
    }

    pub fn add_program_issuance_input(
        &mut self,
        partial_input: PartialInput,
        program_input: ProgramInput,
        issuance_input: IssuanceInput,
        required_sig: RequiredSignature,
    ) -> (AssetId, AssetId) {
        if let RequiredSignature::NativeEcdsa = required_sig {
            panic!("Requested signature is not Witness or None");
        }

        let entropy = asset_entropy(&partial_input.outpoint(), issuance_input.asset_entropy);

        let issuance_asset_id = AssetId::from_entropy(entropy);
        let reissuance_asset_id = AssetId::reissuance_token_from_entropy(entropy, false);

        self.inputs.push(
            FinalInput::new(partial_input, required_sig)
                .with_program(program_input)
                .with_issuance(issuance_input),
        );

        (issuance_asset_id, reissuance_asset_id)
    }

    pub fn add_program_reissuance_input(
        &mut self,
        partial_input: PartialInput,
        program_input: ProgramInput,
        reissuance_input: ReissuanceInput,
        required_sig: RequiredSignature,
    ) -> AssetId {
        if let RequiredSignature::NativeEcdsa = required_sig {
            panic!("Requested signature is not Witness or None");
        }

        if partial_input.secrets.is_none() {
            panic!("Reissuance input must be confidential")
        }

        let asset_entropy = sha256::Midstate::from_byte_array(reissuance_input.asset_entropy);

        let issuance_asset_id = AssetId::from_entropy(asset_entropy);

        self.inputs.push(
            FinalInput::new(partial_input, required_sig)
                .with_program(program_input)
                .with_reissuance(reissuance_input),
        );

        issuance_asset_id
    }

    pub fn remove_input(&mut self, index: usize) -> Option<FinalInput> {
        if self.inputs.get(index).is_some() {
            return Some(self.inputs.remove(index));
        }

        None
    }

    pub fn add_output(&mut self, partial_output: PartialOutput) {
        self.outputs.push(partial_output);
    }

    pub fn remove_output(&mut self, index: usize) -> Option<PartialOutput> {
        if self.outputs.get(index).is_some() {
            return Some(self.outputs.remove(index));
        }

        None
    }

    pub fn inputs(&self) -> &[FinalInput] {
        &self.inputs
    }

    pub fn inputs_mut(&mut self) -> &mut [FinalInput] {
        &mut self.inputs
    }

    pub fn outputs(&self) -> &[PartialOutput] {
        &self.outputs
    }

    pub fn outputs_mut(&mut self) -> &mut [PartialOutput] {
        &mut self.outputs
    }

    pub fn n_inputs(&self) -> usize {
        self.inputs.len()
    }

    pub fn n_outputs(&self) -> usize {
        self.outputs.len()
    }

    pub fn needs_blinding(&self) -> bool {
        self.outputs.iter().any(|el| el.blinding_key.is_some())
    }

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

        available_amount as i64 - consumed_amount as i64
    }

    pub fn calculate_fee(&self, weight: usize, fee_rate: f32) -> u64 {
        let vsize = weight.div_ceil(WITNESS_SCALE_FACTOR);

        (vsize as f32 * fee_rate / 1000.0).ceil() as u64
    }

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
        let issuance = IssuanceInput::new(issuance_amount, entropy);
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
        let reissuance = ReissuanceInput::new(issuance_amount, entropy);
        let partial_output = PartialOutput::new(Script::new(), 1000, policy);

        let mut ft = FinalTransaction::new();
        ft.add_reissuance_input(partial_input.clone(), reissuance.clone(), RequiredSignature::None);
        ft.add_output(partial_output.clone());

        // build expected pst, merge partial_input and issuance manually
        let mut expected_pst = PartiallySignedTransaction::new_v2();
        let mut expected_input = partial_input.to_input();
        let issuance_input = reissuance.to_input();
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
