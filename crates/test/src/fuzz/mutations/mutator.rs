use proptest::test_runner::TestRng;

use rand::RngExt;

use simplicityhl::elements::{AssetId, LockTime, Sequence};

use smplx_sdk::transaction::FinalTransaction;

use crate::fuzz::mutations::{Mutation, Mutations};
use crate::fuzz::outcome::SetupError;
use crate::fuzz::program_scenario::FuzzProgramScenario;

const MUTATIONS_MAX: usize = 3;
const BOUNDARY_PERCENT: u32 = 50;
const LITERAL_PERCENT: u32 = 20;
const FRESH_ASSET_PERCENT: u32 = 50;

type Draw = fn(&Mutator, &FinalTransaction, &mut TestRng) -> Option<Mutation>;

/// Draws the mutations of a case.
#[derive(Clone, Debug)]
pub(crate) struct Mutator {
    mutations: Mutations,
    transaction: FinalTransaction,
    /// Outputs mutated by default: those programs "lock" and the fee outputs.
    target_outputs: Vec<usize>,
    /// Inputs mutated by default: those programs "spend".
    target_inputs: Vec<usize>,
    /// Program literals that fit a `u64`.
    literals: Vec<u64>,
    /// The template's locktime.
    locktime: u32,
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
            target_outputs: Self::target_outputs(transaction, program_scenarios),
            target_inputs: Self::target_inputs(transaction, program_scenarios),
            literals: Self::literals(program_scenarios),
            locktime: Self::locktime(transaction),
        };

        if !mutator.applies(transaction) {
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
            let Some(mutation) = self.draw(&transaction, rng) else {
                break;
            };

            mutation
                .apply(&mut transaction)
                .expect("a drawn mutation fits the transaction it was drawn from");
            mutations.push(mutation);
        }

        (mutations, transaction)
    }

    /// Whether an enabled kind applies to `transaction`. Timelocks always do.
    fn applies(&self, transaction: &FinalTransaction) -> bool {
        self.mutations.timelocks
            || (self.mutations.amounts && !self.transfers(transaction).is_empty())
            || (self.mutations.assets && !self.swaps(transaction).is_empty())
    }

    /// A mutation of a random enabled kind that applies to `transaction`.
    fn draw(&self, transaction: &FinalTransaction, rng: &mut TestRng) -> Option<Mutation> {
        let mut draws = self.draws();

        while !draws.is_empty() {
            let specific_draw = draws.swap_remove(rng.random_range(0..draws.len()));

            if let Some(mutation) = specific_draw(self, transaction, rng) {
                return Some(mutation);
            }
        }

        None
    }

    /// The draws of the enabled kinds.
    fn draws(&self) -> Vec<Draw> {
        let mut draws: Vec<Draw> = Vec::new();

        if self.mutations.amounts {
            draws.push(Self::draw_transfer);
        }

        if self.mutations.assets {
            draws.push(Self::draw_swap);
        }

        if self.mutations.timelocks {
            draws.push(Self::draw_locktime);
            draws.push(Self::draw_sequence);
        }

        draws
    }

    fn draw_transfer(&self, transaction: &FinalTransaction, rng: &mut TestRng) -> Option<Mutation> {
        let transfers = self.transfers(transaction);

        if transfers.is_empty() {
            return None;
        }

        let (from, to) = transfers[rng.random_range(0..transfers.len())];
        let amount = self.draw_amount(transaction.outputs()[from].amount, rng);

        Some(Mutation::TransferValue { from, to, amount })
    }

    /// Another asset of the transaction, or a fresh one.
    fn draw_swap(&self, transaction: &FinalTransaction, rng: &mut TestRng) -> Option<Mutation> {
        let swaps = self.swaps(transaction);

        if swaps.is_empty() {
            return None;
        }

        let output = swaps[rng.random_range(0..swaps.len())];
        let current = transaction.outputs()[output].asset;
        let others: Vec<AssetId> = Self::assets(transaction)
            .into_iter()
            .filter(|asset| *asset != current)
            .collect();

        let asset = if others.is_empty() || rng.random_range(0..100) < FRESH_ASSET_PERCENT {
            AssetId::from_byte_array(rng.random())
        } else {
            others[rng.random_range(0..others.len())]
        };

        Some(Mutation::SwapAsset { output, asset })
    }

    /// The template's locktime or a program literal, or +- 1.
    fn draw_locktime(&self, _: &FinalTransaction, rng: &mut TestRng) -> Option<Mutation> {
        let bases: Vec<u32> = std::iter::once(self.locktime)
            .chain(self.literals.iter().filter_map(|literal| u32::try_from(*literal).ok()))
            .collect();
        let base = bases[rng.random_range(0..bases.len())];

        let locktime = match rng.random_range(0..4) {
            0 => 0,
            1 => base.saturating_sub(1),
            2 => base.saturating_add(1),
            _ => base,
        };

        Some(Mutation::SetLocktime(LockTime::from_consensus(locktime)))
    }

    /// A final or locktime-enabling sequence, or a relative height from a program literal, or +- 1.
    fn draw_sequence(&self, transaction: &FinalTransaction, rng: &mut TestRng) -> Option<Mutation> {
        let inputs: Vec<usize> = (0..transaction.n_inputs())
            .filter(|index| self.mutations.everywhere || self.target_inputs.contains(index))
            .collect();

        if inputs.is_empty() {
            return None;
        }

        let input = inputs[rng.random_range(0..inputs.len())];
        let mut sequences = vec![
            Sequence::MAX,
            Sequence::ENABLE_LOCKTIME_NO_RBF,
            Sequence::ENABLE_RBF_NO_LOCKTIME,
            Sequence::ZERO,
        ];

        for literal in self.literals.iter().filter_map(|literal| u16::try_from(*literal).ok()) {
            sequences
                .extend([literal.saturating_sub(1), literal, literal.saturating_add(1)].map(Sequence::from_height));
        }

        let sequence = sequences[rng.random_range(0..sequences.len())];

        Some(Mutation::SetSequence { input, sequence })
    }

    /// Boundaries (1, all, all but 1), program literals, or a random part of `available`.
    fn draw_amount(&self, available: u64, rng: &mut TestRng) -> u64 {
        let roll = rng.random_range(0..100);
        let literals: Vec<u64> = self
            .literals
            .iter()
            .copied()
            .filter(|amount| (1..=available).contains(amount))
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

    /// Target output pairs value can be transferred between: the same asset and a nonzero amount.
    fn transfers(&self, transaction: &FinalTransaction) -> Vec<(usize, usize)> {
        let outputs = transaction.outputs();
        let targeted = |index: &usize| self.mutations.everywhere || self.target_outputs.contains(index);

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

    /// Targeted outputs with something to pay.
    fn swaps(&self, transaction: &FinalTransaction) -> Vec<usize> {
        (0..transaction.n_outputs())
            .filter(|index| self.mutations.everywhere || self.target_outputs.contains(index))
            .filter(|index| transaction.outputs()[*index].amount > 0)
            .collect()
    }

    /// The assets of the inputs and outputs.
    fn assets(transaction: &FinalTransaction) -> Vec<AssetId> {
        let inputs = transaction.inputs().iter().filter_map(|input| {
            let input = &input.partial_input;

            input.secrets.map(|secrets| secrets.asset).or(input.asset)
        });
        let mut assets: Vec<AssetId> = inputs
            .chain(transaction.outputs().iter().map(|output| output.asset))
            .collect();

        assets.sort_unstable();
        assets.dedup();

        assets
    }

    /// Outputs a program locks and the fee outputs.
    fn target_outputs(transaction: &FinalTransaction, program_scenarios: &[FuzzProgramScenario]) -> Vec<usize> {
        (0..transaction.n_outputs())
            .filter(|index| {
                transaction.outputs()[*index].script_pubkey.is_empty()
                    || program_scenarios
                        .iter()
                        .any(|program_scenario| program_scenario.outputs.contains(index))
            })
            .collect()
    }

    /// Inputs a program spends.
    fn target_inputs(transaction: &FinalTransaction, program_scenarios: &[FuzzProgramScenario]) -> Vec<usize> {
        (0..transaction.n_inputs())
            .filter(|index| {
                program_scenarios
                    .iter()
                    .any(|program_scenario| program_scenario.inputs.contains(index))
            })
            .collect()
    }

    /// Program literals that fit a `u64`.
    fn literals(program_scenarios: &[FuzzProgramScenario]) -> Vec<u64> {
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
            .collect()
    }

    fn locktime(transaction: &FinalTransaction) -> u32 {
        transaction
            .extract_pst()
            .0
            .extract_tx()
            .map_or(0, |transaction| transaction.lock_time.to_consensus_u32())
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
