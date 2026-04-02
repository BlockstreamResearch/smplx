use simplicityhl::elements::AssetId;
use simplicityhl::elements::pset::PartiallySignedTransaction;

use crate::provider::SimplicityNetwork;
use crate::utils::asset_entropy;

use super::error::TransactionError;
use super::partial_input::{IssuanceInput, PartialInput, ProgramInput, RequiredSignature};
use super::partial_output::PartialOutput;

pub const WITNESS_SCALE_FACTOR: usize = 4;

#[derive(Clone)]
pub struct FinalInput {
    pub partial_input: PartialInput,
    pub program_input: Option<ProgramInput>,
    pub issuance_input: Option<IssuanceInput>,
    pub required_sig: RequiredSignature,
}

#[derive(Clone)]
pub struct FinalTransaction {
    pub network: SimplicityNetwork,
    inputs: Vec<FinalInput>,
    outputs: Vec<PartialOutput>,
}

impl FinalTransaction {
    pub fn new(network: SimplicityNetwork) -> Self {
        Self {
            network,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    pub fn add_input(
        &mut self,
        partial_input: PartialInput,
        required_sig: RequiredSignature,
    ) -> Result<(), TransactionError> {
        if let RequiredSignature::Witness(_) = required_sig {
            return Err(TransactionError::SignatureRequest(
                "Requested signature is not NativeEcdsa or None".to_string(),
            ));
        }

        self.inputs.push(FinalInput {
            partial_input,
            program_input: None,
            issuance_input: None,
            required_sig,
        });

        Ok(())
    }

    pub fn add_program_input(
        &mut self,
        partial_input: PartialInput,
        program_input: ProgramInput,
        required_sig: RequiredSignature,
    ) -> Result<(), TransactionError> {
        if let RequiredSignature::NativeEcdsa = required_sig {
            return Err(TransactionError::SignatureRequest(
                "Requested signature is not Witness or None".to_string(),
            ));
        }

        self.inputs.push(FinalInput {
            partial_input,
            program_input: Some(program_input),
            issuance_input: None,
            required_sig,
        });

        Ok(())
    }

    pub fn add_issuance_input(
        &mut self,
        partial_input: PartialInput,
        issuance_input: IssuanceInput,
        required_sig: RequiredSignature,
    ) -> Result<AssetId, TransactionError> {
        if let RequiredSignature::Witness(_) = required_sig {
            return Err(TransactionError::SignatureRequest(
                "Requested signature is not NativeEcdsa or None".to_string(),
            ));
        }

        let asset_id = AssetId::from_entropy(asset_entropy(&partial_input.outpoint(), issuance_input.asset_entropy));

        self.inputs.push(FinalInput {
            partial_input,
            program_input: None,
            issuance_input: Some(issuance_input),
            required_sig,
        });

        Ok(asset_id)
    }

    pub fn add_program_issuance_input(
        &mut self,
        partial_input: PartialInput,
        program_input: ProgramInput,
        issuance_input: IssuanceInput,
        required_sig: RequiredSignature,
    ) -> Result<AssetId, TransactionError> {
        if let RequiredSignature::NativeEcdsa = required_sig {
            return Err(TransactionError::SignatureRequest(
                "Requested signature is not Witness or None".to_string(),
            ));
        }

        let asset_id = AssetId::from_entropy(asset_entropy(&partial_input.outpoint(), issuance_input.asset_entropy));

        self.inputs.push(FinalInput {
            partial_input,
            program_input: Some(program_input),
            issuance_input: Some(issuance_input),
            required_sig,
        });

        Ok(asset_id)
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

    pub fn calculate_fee_delta(&self) -> i64 {
        let available_amount = self
            .inputs
            .iter()
            .filter(|input| input.partial_input.asset.unwrap() == self.network.policy_asset())
            .fold(0_u64, |acc, input| acc + input.partial_input.amount.unwrap());

        let consumed_amount = self
            .outputs
            .iter()
            .filter(|output| output.asset == self.network.policy_asset())
            .fold(0_u64, |acc, output| acc + output.amount);

        available_amount as i64 - consumed_amount as i64
    }

    pub fn calculate_fee(&self, weight: usize, fee_rate: f32) -> u64 {
        let vsize = weight.div_ceil(WITNESS_SCALE_FACTOR);

        (vsize as f32 * fee_rate / 1000.0).ceil() as u64
    }

    pub fn extract_pst(&self) -> PartiallySignedTransaction {
        let mut pst = PartiallySignedTransaction::new_v2();

        self.inputs.iter().for_each(|el| {
            let mut input = el.partial_input.input();

            // populate the input manually since `input.merge` is private
            if el.issuance_input.is_some() {
                let issue = el.issuance_input.clone().unwrap().input();

                input.issuance_value_amount = issue.issuance_value_amount;
                input.issuance_asset_entropy = issue.issuance_asset_entropy;
                input.issuance_inflation_keys = issue.issuance_inflation_keys;
                input.blinded_issuance = issue.blinded_issuance;
            }

            pst.add_input(input);
        });

        self.outputs.iter().for_each(|el| {
            pst.add_output(el.to_output());
        });

        pst
    }
}
#[cfg(test)]
mod tests {
    use bitcoin_hashes::Hash;
    use simplicityhl::elements::{OutPoint, Script, TxOut, TxOutWitness, Txid, confidential};

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
            txout: TxOut {
                asset: confidential::Asset::Null,
                value: confidential::Value::Null,
                nonce: confidential::Nonce::Null,
                script_pubkey: Script::new(),
                witness: TxOutWitness::default(),
            },
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
}
