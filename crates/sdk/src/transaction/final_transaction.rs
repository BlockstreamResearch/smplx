use simplicityhl::elements::pset::PartiallySignedTransaction;

use crate::constants::{SimplicityNetwork, WITNESS_SCALE_FACTOR};

use super::error::TransactionError;
use super::partial_input::{PartialInput, ProgramInput, RequiredSignature};
use super::partial_output::PartialOutput;

#[derive(Clone)]
pub struct FinalInput {
    pub partial_input: PartialInput,
    pub program_input: Option<ProgramInput>,
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

    // TODO: require required_sig != Witness(String)
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
            required_sig: required_sig,
        });

        Ok(())
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

    pub fn calculate_fee_delta(&self) -> u64 {
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

        available_amount - consumed_amount
    }

    pub fn calculate_fee(&self, weight: usize, fee_rate: f32) -> u64 {
        let vsize = weight.div_ceil(WITNESS_SCALE_FACTOR);

        (vsize as f32 * fee_rate / 1000.0).ceil() as u64
    }

    pub fn extract_pst(&self) -> PartiallySignedTransaction {
        let mut pst = PartiallySignedTransaction::new_v2();

        self.inputs.iter().for_each(|el| {
            pst.add_input(el.partial_input.to_input());
        });

        self.outputs.iter().for_each(|el| {
            pst.add_output(el.to_output());
        });

        pst
    }
}
