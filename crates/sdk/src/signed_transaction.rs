use simplicityhl::elements::pset::{Input, Output, PartiallySignedTransaction};
use simplicityhl::elements::secp256k1_zkp::schnorr::Signature;
use simplicityhl::elements::{Address, Script, Transaction, TxInWitness, TxOut, script, taproot};
use simplicityhl::{Value, WitnessValues};

use crate::constants::SimplicityNetwork;
use crate::error::SignerError;
use crate::program::{ProgramTrait, WitnessTrait};
use crate::signer::SignerTrait;

struct SignedInput<'a, T> {
    program: &'a dyn ProgramTrait,
    witness: &'a dyn WitnessTrait,
    signer: Option<&'a dyn SignerTrait>,
    signer_lambda: Option<T>,
}

pub struct SignedTransaction<'a, T> {
    tx: Transaction,
    utxos: &'a [TxOut],
    network: SimplicityNetwork,
    inputs: Vec<SignedInput<'a, T>>,
}

impl<'a, T> SignedTransaction<'a, T>
where
    T: Fn(WitnessValues, Signature) -> Result<WitnessValues, SignerError> + Clone,
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

    pub fn finalize(&mut self) -> Result<Transaction, SignerError> {
        for index in 0..self.inputs.len() {
            let (program, witness, signer, signer_lambda) = {
                let input = &self.inputs[index];
                (input.program, input.witness, input.signer, input.signer_lambda.clone())
            };

            if signer.is_some() {
                self.finalize_with_signer(
                    program,
                    witness.build_witness(),
                    index,
                    signer.unwrap(),
                    signer_lambda.unwrap(),
                )?;
            } else {
                self.finalize_as_is(program, witness.build_witness(), index)?;
            }
        }

        Ok(self.tx.clone())
    }

    fn finalize_with_signer(
        &mut self,
        program: &dyn ProgramTrait,
        witness: WitnessValues,
        index: usize,
        signer: &dyn SignerTrait,
        signer_lambda: T,
    ) -> Result<(), SignerError> {
        let signature = signer.sign(program, &self.tx, self.utxos, index, self.network)?;
        let new_witness = signer_lambda(witness, signature)?;

        self.finalize_as_is(program, new_witness, index)
    }

    fn finalize_as_is(
        &mut self,
        program: &dyn ProgramTrait,
        witness: WitnessValues,
        index: usize,
    ) -> Result<(), SignerError> {
        self.tx = program.finalize(witness, self.tx.clone(), self.utxos, index, self.network)?;

        Ok(())
    }
}
