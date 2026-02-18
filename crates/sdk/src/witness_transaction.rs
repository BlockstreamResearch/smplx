use simplicityhl::WitnessValues;
use simplicityhl::elements::secp256k1_zkp::schnorr::Signature;
use simplicityhl::elements::{Transaction, TxOut};

use crate::constants::{SimplicityNetwork, WITNESS_SCALE_FACTOR};
use crate::error::SimplexError;
use crate::program::ProgramTrait;
use crate::provider::Provider;
use crate::signer::SignerTrait;
use crate::witness::WitnessTrait;

struct SignedInput<'a, T> {
    program: &'a dyn ProgramTrait,
    witness: &'a dyn WitnessTrait,
    signer: Option<&'a dyn SignerTrait>,
    signer_lambda: Option<T>,
}

pub struct WitnessTransaction<'a, T> {
    tx: Transaction,
    utxos: &'a [TxOut],
    network: SimplicityNetwork,
    inputs: Vec<SignedInput<'a, T>>,
}

impl<'a, T> WitnessTransaction<'a, T>
where
    T: Fn(&WitnessValues, &Signature) -> Result<WitnessValues, SimplexError> + Clone,
{
    pub fn new(tx: Transaction, utxos: &'a [TxOut], network: SimplicityNetwork) -> Self {
        Self {
            tx: tx,
            utxos: utxos,
            network: network,
            inputs: Vec::new(),
        }
    }

    pub fn add_input(&mut self, program: &'a dyn ProgramTrait, witness: &'a dyn WitnessTrait) {
        let signed_input = SignedInput {
            program: program,
            witness: witness,
            signer: Option::None,
            signer_lambda: Option::None,
        };

        self.inputs.push(signed_input);
    }

    pub fn add_signed_input(
        &mut self,
        program: &'a dyn ProgramTrait,
        witness: &'a dyn WitnessTrait,
        signer: &'a dyn SignerTrait,
        signer_lambda: T,
    ) {
        let signed_input = SignedInput {
            program: program,
            witness: witness,
            signer: Option::Some(signer),
            signer_lambda: Option::Some(signer_lambda),
        };

        self.inputs.push(signed_input);
    }

    pub fn finalize_with_fee(
        &self,
        target_blocks: u32,
        provider: impl Provider,
    ) -> Result<(Transaction, u64), SimplexError> {
        let fee_rate = provider.get_fee_rate(target_blocks)?;
        let final_tx = self.finalize()?;

        let fee = self.calculate_fee(final_tx.weight(), fee_rate);

        Ok((final_tx, fee))
    }

    pub fn finalize(&self) -> Result<Transaction, SimplexError> {
        let mut final_tx = self.tx.clone();

        for index in 0..self.inputs.len() {
            let (program, witness, signer, signer_lambda) = {
                let input = &self.inputs[index];
                (input.program, input.witness, input.signer, input.signer_lambda.clone())
            };

            if signer.is_some() {
                final_tx = self.finalize_with_signer(
                    final_tx,
                    program,
                    witness.build_witness(),
                    index,
                    signer.unwrap(),
                    signer_lambda.unwrap(),
                )?;
            } else {
                final_tx = self.finalize_as_is(final_tx, program, witness.build_witness(), index)?;
            }
        }

        Ok(final_tx)
    }

    fn finalize_with_signer(
        &self,
        final_tx: Transaction,
        program: &dyn ProgramTrait,
        witness: WitnessValues,
        index: usize,
        signer: &dyn SignerTrait,
        signer_lambda: T,
    ) -> Result<Transaction, SimplexError> {
        let signature = signer.sign(program, &final_tx, self.utxos, index, self.network)?;
        let new_witness = signer_lambda(&witness, &signature)?;

        Ok(self.finalize_as_is(final_tx, program, new_witness, index)?)
    }

    fn finalize_as_is(
        &self,
        final_tx: Transaction,
        program: &dyn ProgramTrait,
        witness: WitnessValues,
        index: usize,
    ) -> Result<Transaction, SimplexError> {
        Ok(program.finalize(witness, final_tx, self.utxos, index, self.network)?)
    }

    fn calculate_fee(&self, weight: usize, fee_rate: f32) -> u64 {
        let vsize = weight.div_ceil(WITNESS_SCALE_FACTOR);
        (vsize as f32 * fee_rate / 1000.0).ceil() as u64
    }
}
