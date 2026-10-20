use proptest::test_runner::TestRng;

use rand::RngExt;

use smplx_sdk::transaction::FinalTransaction;

use crate::fuzz::mutations::{Mutation, Mutations};
use crate::fuzz::outcome::SetupError;
use crate::fuzz::program_scenario::FuzzProgramScenario;

const MUTATIONS_MAX: usize = 3;
const BOUNDARY_PERCENT: u32 = 50;
const LITERAL_PERCENT: u32 = 20;

/// Draws the mutations of a case.
#[derive(Clone, Debug)]
pub(crate) struct Mutator {
    mutations: Mutations,
    transaction: FinalTransaction,
    targets: Vec<usize>,
    amounts: Vec<u64>,
}

impl Mutator {
    /// # Errors
    /// Returns a [`SetupError`] when `transaction` is not a balanced transaction of explicit or unblinded UTXOs,
    /// or no enabled mutation applies to it.
    pub(crate) fn new(
        mutations: Mutations,
        transaction: &FinalTransaction,
        program_scenarios: &[FuzzProgramScenario],
    ) -> Result<Self, SetupError> {
        let known = transaction.inputs().iter().all(|input| {
            let input = &input.partial_input;

            input.secrets.is_some() || (input.asset.is_some() && input.amount.is_some())
        });

        if !known || !transaction.is_balanced() {
            return Err(SetupError::Transaction(
                "mutations need a balanced transaction of explicit or unblinded UTXOs".to_string(),
            ));
        }

        let mutator = Self {
            mutations,
            transaction: transaction.clone(),
            targets: Self::targets(transaction, program_scenarios),
            amounts: Self::amounts(program_scenarios),
        };

        if mutator.transfers(transaction).is_empty() {
            return Err(SetupError::Transaction(
                "no mutation applies to the transaction, e.g. transferring value needs two outputs of the same asset"
                    .to_string(),
            ));
        }

        Ok(mutator)
    }

    /// Draws one to `MUTATIONS_MAX` mutations and the transaction they make.
    pub(crate) fn generate(&self, rng: &mut TestRng) -> (Vec<Mutation>, FinalTransaction) {
        let mut transaction = self.transaction.clone();
        let mut mutations = Vec::new();

        for _ in 0..rng.random_range(1..=MUTATIONS_MAX) {
            let Some(mutation) = self.draw_transfer(&transaction, rng) else {
                break;
            };

            mutation
                .apply(&mut transaction)
                .expect("a drawn mutation fits the transaction it was drawn from");
            mutations.push(mutation);
        }

        (mutations, transaction)
    }

    fn draw_transfer(&self, transaction: &FinalTransaction, rng: &mut TestRng) -> Option<Mutation> {
        let transfers = self.transfers(transaction);

        if transfers.is_empty() {
            return None;
        }

        let (from, to) = transfers[rng.random_range(0..transfers.len())];
        let amount = self.draw_amount(transaction.outputs()[from].amount, rng);

        Some(Mutation::MoveValue { from, to, amount })
    }

    /// Targeted output pairs value can be transferred between: the same asset and a nonzero source.
    fn transfers(&self, transaction: &FinalTransaction) -> Vec<(usize, usize)> {
        let outputs = transaction.outputs();
        let targeted = |index: &usize| self.mutations.everywhere || self.targets.contains(index);

        (0..outputs.len())
            .flat_map(|from| (0..outputs.len()).map(move |to| (from, to)))
            .filter(|&(from, to)| {
                from != to
                    && outputs[from].asset == outputs[to].asset
                    && outputs[from].amount > 0
                    && (targeted(&from) || targeted(&to))
            })
            .collect()
    }

    /// Boundaries (1, all, all - 1), program literals, or a random part of `available`.
    fn draw_amount(&self, available: u64, rng: &mut TestRng) -> u64 {
        let roll = rng.random_range(0..100);
        let literals: Vec<u64> = self
            .amounts
            .iter()
            .copied()
            .filter(|amount| *amount <= available)
            .collect();

        if roll < BOUNDARY_PERCENT {
            let boundaries: Vec<u64> = [1, available, available - 1]
                .into_iter()
                .filter(|amount| *amount > 0)
                .collect();

            boundaries[rng.random_range(0..boundaries.len())]
        } else if roll < BOUNDARY_PERCENT + LITERAL_PERCENT && !literals.is_empty() {
            literals[rng.random_range(0..literals.len())]
        } else {
            rng.random_range(1..=available)
        }
    }

    /// Outputs a program locks and the fee outputs.
    fn targets(transaction: &FinalTransaction, program_scenarios: &[FuzzProgramScenario]) -> Vec<usize> {
        (0..transaction.n_outputs())
            .filter(|index| {
                transaction.outputs()[*index].script_pubkey.is_empty()
                    || program_scenarios
                        .iter()
                        .any(|program_scenario| program_scenario.outputs.contains(index))
            })
            .collect()
    }

    /// Program literals that fit a nonzero amount.
    fn amounts(program_scenarios: &[FuzzProgramScenario]) -> Vec<u64> {
        program_scenarios
            .iter()
            .filter_map(|program_scenario| program_scenario.strategy.schema())
            .flat_map(|schema| &schema.literals)
            .filter_map(|literal| {
                let bytes = literal.to_byte_array();

                bytes[..24]
                    .iter()
                    .all(|byte| *byte == 0)
                    .then(|| u64::from_be_bytes(bytes[24..].try_into().expect("8 bytes")))
            })
            .filter(|amount| *amount > 0)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use simplicityhl::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
    use simplicityhl::elements::{AssetId, OutPoint, Script, TxOut, TxOutSecrets};

    use smplx_sdk::transaction::{PartialInput, PartialOutput, RequiredSignature, UTXO};

    use super::*;

    #[test]
    fn mutates_an_unblinded_input() {
        let asset = AssetId::default();
        let secrets = TxOutSecrets::new(asset, AssetBlindingFactor::zero(), 10, ValueBlindingFactor::zero());

        let mut transaction = FinalTransaction::new();
        transaction.add_input(
            PartialInput::new(UTXO {
                outpoint: OutPoint::default(),
                txout: TxOut::default(),
                secrets: Some(secrets),
            }),
            RequiredSignature::None,
        );
        transaction.add_output(PartialOutput::new(Script::new(), 10, asset));
        transaction.add_output(PartialOutput::default());

        assert!(Mutator::new(Mutations::default(), &transaction, &[]).is_ok());
    }
}
