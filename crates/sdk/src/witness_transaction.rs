use simplicityhl::WitnessValues;
use simplicityhl::elements::pset::{Output, PartiallySignedTransaction};
use simplicityhl::elements::secp256k1_zkp::schnorr::Signature;
use simplicityhl::elements::{Script, Transaction, TxOut};

use crate::constants::{MIN_FEE, PLACEHOLDER_FEE, SimplicityNetwork, WITNESS_SCALE_FACTOR};
use crate::error::SimplexError;
use crate::program::ProgramTrait;
use crate::provider::ProviderSync;
use crate::signer::SignerTrait;
use crate::witness::WitnessTrait;

struct SignedInput<'a, T> {
    program: &'a dyn ProgramTrait,
    witness: &'a dyn WitnessTrait,
    signer: Option<&'a dyn SignerTrait>,
    signer_lambda: Option<T>,
}

pub struct WitnessTransaction<'a, T> {
    pst: PartiallySignedTransaction,
    network: SimplicityNetwork,
    inputs: Vec<SignedInput<'a, T>>,
}

impl<'a, T> WitnessTransaction<'a, T>
where
    T: Fn(&WitnessValues, &Signature) -> Result<WitnessValues, SimplexError> + Clone,
{
    pub fn new(pst: PartiallySignedTransaction, network: SimplicityNetwork) -> Self {
        Self {
            pst: pst,
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
        change_recipient_script: Script,
        provider: impl ProviderSync,
    ) -> Result<(Transaction, u64), SimplexError> {
        let policy_amount_delta = self.calculate_fee_delta();

        if policy_amount_delta < MIN_FEE {
            return Err(SimplexError::DustAmount(policy_amount_delta));
        }

        // estimate the tx fee with the change
        let fee_rate = provider.get_fee_rate(target_blocks)?;
        let mut fee_pst = self.pst.clone();

        fee_pst.add_output(Output::new_explicit(
            change_recipient_script.clone(),
            PLACEHOLDER_FEE,
            self.network.policy_asset(),
            None,
        ));

        fee_pst.add_output(Output::new_explicit(
            Script::new(),
            PLACEHOLDER_FEE,
            self.network.policy_asset(),
            None,
        ));

        let (final_tx, utxos) = self.extract_tx_and_utxos(&fee_pst)?;
        let final_tx = self.finalize_tx(final_tx, utxos.as_slice())?;
        let fee = self.calculate_fee(final_tx.weight(), fee_rate);

        if policy_amount_delta > fee && policy_amount_delta - fee >= MIN_FEE {
            // we have enough funds to cover change UTXO
            let mut fee_pst = self.pst.clone();

            fee_pst.add_output(Output::new_explicit(
                change_recipient_script,
                policy_amount_delta - fee,
                self.network.policy_asset(),
                None,
            ));

            fee_pst.add_output(Output::new_explicit(
                Script::new(),
                fee,
                self.network.policy_asset(),
                None,
            ));

            let (final_tx, utxos) = self.extract_tx_and_utxos(&fee_pst)?;
            let final_tx = self.finalize_tx(final_tx, utxos.as_slice())?;

            return Ok((final_tx, fee));
        }

        // not enough funds, so we need to estimate without the change
        let mut fee_pst = self.pst.clone();

        fee_pst.add_output(Output::new_explicit(
            Script::new(),
            PLACEHOLDER_FEE,
            self.network.policy_asset(),
            None,
        ));

        let (final_tx, utxos) = self.extract_tx_and_utxos(&fee_pst)?;
        let final_tx = self.finalize_tx(final_tx, utxos.as_slice())?;
        let fee = self.calculate_fee(final_tx.weight(), fee_rate);

        // policy amount is not exact
        if policy_amount_delta != fee {
            return Err(SimplexError::NotEnoughFeeAmount(policy_amount_delta, fee));
        }

        // finalize the tx with fee and without the change
        let mut fee_pst = self.pst.clone();

        fee_pst.add_output(Output::new_explicit(
            Script::new(),
            fee,
            self.network.policy_asset(),
            None,
        ));

        let (final_tx, utxos) = self.extract_tx_and_utxos(&fee_pst)?;
        let final_tx = self.finalize_tx(final_tx, utxos.as_slice())?;

        Ok((final_tx, fee))
    }

    pub fn finalize(&self) -> Result<Transaction, SimplexError> {
        let (final_tx, utxos) = self.extract_tx_and_utxos(&self.pst)?;

        Ok(self.finalize_tx(final_tx, utxos.as_slice())?)
    }

    fn finalize_tx(&self, mut final_tx: Transaction, utxos: &[TxOut]) -> Result<Transaction, SimplexError> {
        for index in 0..self.inputs.len() {
            let (program, witness, signer, signer_lambda) = {
                let input = &self.inputs[index];
                (input.program, input.witness, input.signer, input.signer_lambda.clone())
            };

            if signer.is_some() {
                final_tx = self.finalize_tx_with_signer(
                    final_tx,
                    utxos,
                    program,
                    witness.build_witness(),
                    index,
                    signer.unwrap(),
                    signer_lambda.unwrap(),
                )?;
            } else {
                final_tx = self.finalize_tx_as_is(final_tx, utxos, program, witness.build_witness(), index)?;
            }
        }

        Ok(final_tx)
    }

    #[allow(clippy::too_many_arguments)]
    fn finalize_tx_with_signer(
        &self,
        final_tx: Transaction,
        utxos: &[TxOut],
        program: &dyn ProgramTrait,
        witness: WitnessValues,
        index: usize,
        signer: &dyn SignerTrait,
        signer_lambda: T,
    ) -> Result<Transaction, SimplexError> {
        let signature = signer.sign(program, &final_tx, utxos, index, self.network)?;
        let new_witness = signer_lambda(&witness, &signature)?;

        self.finalize_tx_as_is(final_tx, utxos, program, new_witness, index)
    }

    fn finalize_tx_as_is(
        &self,
        final_tx: Transaction,
        utxos: &[TxOut],
        program: &dyn ProgramTrait,
        witness: WitnessValues,
        index: usize,
    ) -> Result<Transaction, SimplexError> {
        program.finalize(witness, final_tx, utxos, index, self.network)
    }

    fn calculate_fee_delta(&self) -> u64 {
        let available_amount = self
            .pst
            .inputs()
            .iter()
            .filter(|input| input.asset.unwrap() == self.network.policy_asset())
            .fold(0_u64, |acc, input| acc + input.amount.unwrap());

        let consumed_amount = self
            .pst
            .outputs()
            .iter()
            .filter(|output| output.asset.unwrap() == self.network.policy_asset())
            .fold(0_u64, |acc, output| acc + output.amount.unwrap());

        available_amount - consumed_amount
    }

    fn calculate_fee(&self, weight: usize, fee_rate: f32) -> u64 {
        let vsize = weight.div_ceil(WITNESS_SCALE_FACTOR);
        (vsize as f32 * fee_rate / 1000.0).ceil() as u64
    }

    fn extract_tx_and_utxos(
        &self,
        pst: &PartiallySignedTransaction,
    ) -> Result<(Transaction, Vec<TxOut>), SimplexError> {
        let final_tx = pst.extract_tx()?;
        let mut utxos: Vec<TxOut> = vec![];

        for input in pst.inputs() {
            utxos.push(input.witness_utxo.clone().unwrap());
        }

        Ok((final_tx, utxos))
    }
}
