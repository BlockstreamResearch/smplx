use std::collections::HashMap;

use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::{
    AssetId, TxOutSecrets,
    confidential::{AssetBlindingFactor, ValueBlindingFactor},
};

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
            let mut pst_input = final_input.partial_input.to_input();

            // populate the input manually since `input.merge` is private
            if final_input.issuance_input.is_some() {
                let issue = final_input.issuance_input.clone().unwrap().to_input();

                pst_input.issuance_value_amount = issue.issuance_value_amount;
                pst_input.issuance_asset_entropy = issue.issuance_asset_entropy;
                pst_input.issuance_inflation_keys = issue.issuance_inflation_keys;
                pst_input.blinded_issuance = issue.blinded_issuance;
            }

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
