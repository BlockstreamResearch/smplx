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
            network: network,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    pub fn add_input(
        &mut self,
        partial_input: PartialInput,
        required_sig: RequiredSignature,
    ) -> Result<(), TransactionError> {
        match required_sig {
            RequiredSignature::Witness(_) => {
                return Err(TransactionError::SignatureRequest(
                    "Requested signature is not NativeEcdsa or None".to_string(),
                ));
            }
            _ => {}
        }

        self.inputs.push(FinalInput {
            partial_input: partial_input,
            program_input: None,
            issuance_input: None,
            required_sig: required_sig,
        });

        Ok(())
    }

    pub fn add_program_input(
        &mut self,
        partial_input: PartialInput,
        program_input: ProgramInput,
        required_sig: RequiredSignature,
    ) -> Result<(), TransactionError> {
        match required_sig {
            RequiredSignature::NativeEcdsa => {
                return Err(TransactionError::SignatureRequest(
                    "Requested signature is not Witness or None".to_string(),
                ));
            }
            _ => {}
        }

        self.inputs.push(FinalInput {
            partial_input: partial_input,
            program_input: Some(program_input),
            issuance_input: None,
            required_sig: required_sig,
        });

        Ok(())
    }

    pub fn add_issuance_input(
        &mut self,
        partial_input: PartialInput,
        issuance_input: IssuanceInput,
        required_sig: RequiredSignature,
    ) -> Result<AssetId, TransactionError> {
        match required_sig {
            RequiredSignature::Witness(_) => {
                return Err(TransactionError::SignatureRequest(
                    "Requested signature is not NativeEcdsa or None".to_string(),
                ));
            }
            _ => {}
        }

        let asset_id = AssetId::from_entropy(asset_entropy(&partial_input.outpoint(), issuance_input.asset_entropy));

        self.inputs.push(FinalInput {
            partial_input: partial_input,
            program_input: None,
            issuance_input: Some(issuance_input),
            required_sig: required_sig,
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
        match required_sig {
            RequiredSignature::NativeEcdsa => {
                return Err(TransactionError::SignatureRequest(
                    "Requested signature is not Witness or None".to_string(),
                ));
            }
            _ => {}
        }

        let asset_id = AssetId::from_entropy(asset_entropy(&partial_input.outpoint(), issuance_input.asset_entropy));

        self.inputs.push(FinalInput {
            partial_input: partial_input,
            program_input: Some(program_input),
            issuance_input: Some(issuance_input),
            required_sig: required_sig,
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
            .filter(|input| input.partial_input.asset.clone().unwrap() == self.network.policy_asset())
            .fold(0 as u64, |acc, input| acc + input.partial_input.amount.clone().unwrap());

        let consumed_amount = self
            .outputs
            .iter()
            .filter(|output| output.asset == self.network.policy_asset())
            .fold(0 as u64, |acc, output| acc + output.amount);

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
