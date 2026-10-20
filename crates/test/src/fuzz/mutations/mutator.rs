use proptest::test_runner::TestRng;

use rand::RngExt;

use simplicityhl::elements::{AssetId, LockTime, Sequence};

use smplx_sdk::transaction::FinalTransaction;

use crate::fuzz::mutations::{Mutation, Mutations, UtxoMap};
use crate::fuzz::outcome::SetupError;
use crate::fuzz::program_scenario::FuzzProgramScenario;
use crate::fuzz::program_strategy::Word;

const MUTATIONS_MAX: usize = 3;
const BOUNDARY_PERCENT: u32 = 50;
const LITERAL_PERCENT: u32 = 20;
const FRESH_ASSET_PERCENT: u32 = 50;

type Draw = fn(&Mutator, &FinalTransaction, &UtxoMap, &mut TestRng) -> Option<Mutation>;

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
            target_outputs: Self::find_target_outputs(transaction, program_scenarios),
            target_inputs: Self::find_target_inputs(transaction, program_scenarios),
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
        let mut map = UtxoMap::new(&transaction, &[]);
        let mut mutations = Vec::new();

        for _ in 0..rng.random_range(1..=MUTATIONS_MAX) {
            let Some(mutation) = self.draw(&transaction, &map, rng) else {
                break;
            };

            mutation
                .apply(&mut transaction)
                .expect("a drawn mutation fits the transaction it was drawn from");
            map.follow(&mutation);
            mutations.push(mutation);
        }

        (mutations, transaction)
    }

    /// Whether an enabled kind applies to `transaction`. Timelocks and layout always do.
    fn applies(&self, transaction: &FinalTransaction) -> bool {
        let map = UtxoMap::new(transaction, &[]);

        self.mutations.timelocks
            || self.mutations.layout
            || (self.mutations.amounts && !self.transfers(transaction, &map).is_empty())
            || (self.mutations.assets && !self.funded_outputs(transaction, &map).is_empty())
    }

    /// A mutation of a random enabled kind that applies to `transaction`.
    fn draw(&self, transaction: &FinalTransaction, map: &UtxoMap, rng: &mut TestRng) -> Option<Mutation> {
        let mut draws = self.draws();

        while !draws.is_empty() {
            let specific_draw = draws.swap_remove(rng.random_range(0..draws.len()));

            if let Some(mutation) = specific_draw(self, transaction, map, rng) {
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

        if self.mutations.layout {
            draws.push(Self::draw_add_output);
            draws.push(Self::draw_add_input);
            draws.push(Self::draw_swap_inputs);
            draws.push(Self::draw_swap_outputs);
        }

        draws
    }

    fn draw_transfer(&self, transaction: &FinalTransaction, map: &UtxoMap, rng: &mut TestRng) -> Option<Mutation> {
        let transfers = self.transfers(transaction, map);

        if transfers.is_empty() {
            return None;
        }

        let (from, to) = transfers[rng.random_range(0..transfers.len())];
        let amount = self.draw_amount(transaction.outputs()[from].amount, rng);

        Some(Mutation::TransferValue { from, to, amount })
    }

    fn draw_swap(&self, transaction: &FinalTransaction, map: &UtxoMap, rng: &mut TestRng) -> Option<Mutation> {
        let swaps = self.funded_outputs(transaction, map);

        if swaps.is_empty() {
            return None;
        }

        let output = swaps[rng.random_range(0..swaps.len())];
        let asset = Self::draw_asset(transaction, Some(transaction.outputs()[output].asset), rng);

        Some(Mutation::SwapAsset { output, asset })
    }

    /// The template's locktime or a program literal, or +- 1.
    fn draw_locktime(&self, _: &FinalTransaction, _: &UtxoMap, rng: &mut TestRng) -> Option<Mutation> {
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
    fn draw_sequence(&self, transaction: &FinalTransaction, map: &UtxoMap, rng: &mut TestRng) -> Option<Mutation> {
        let inputs: Vec<usize> = (0..transaction.n_inputs())
            .filter(|position| self.targets_input(*position, map))
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

    /// Takes part of a targeted output for an attacker output.
    fn draw_add_output(&self, transaction: &FinalTransaction, map: &UtxoMap, rng: &mut TestRng) -> Option<Mutation> {
        let sources = self.funded_outputs(transaction, map);

        if sources.is_empty() {
            return None;
        }

        let from = sources[rng.random_range(0..sources.len())];
        let amount = self.draw_amount(transaction.outputs()[from].amount, rng);

        Some(Mutation::AddOutput { from, amount })
    }

    /// An attacker UTXO in an asset of the transaction or a fresh one, worth up to the largest output.
    fn draw_add_input(&self, transaction: &FinalTransaction, _: &UtxoMap, rng: &mut TestRng) -> Option<Mutation> {
        let largest = transaction
            .outputs()
            .iter()
            .map(|output| output.amount)
            .max()
            .unwrap_or(0);

        Some(Mutation::AddInput {
            asset: Self::draw_asset(transaction, None, rng),
            amount: self.draw_amount(largest.max(1), rng),
        })
    }

    fn draw_swap_inputs(&self, transaction: &FinalTransaction, map: &UtxoMap, rng: &mut TestRng) -> Option<Mutation> {
        let (first, second) = Self::draw_pair(
            transaction.n_inputs(),
            |position| self.targets_input(position, map),
            rng,
        )?;

        Some(Mutation::SwapInputs(first, second))
    }

    fn draw_swap_outputs(&self, transaction: &FinalTransaction, map: &UtxoMap, rng: &mut TestRng) -> Option<Mutation> {
        let (first, second) = Self::draw_pair(
            transaction.n_outputs(),
            |position| self.targets_output(position, map),
            rng,
        )?;

        Some(Mutation::SwapOutputs(first, second))
    }

    /// Two of `count` positions, at least one of them targeted.
    fn draw_pair(count: usize, targeted: impl Fn(usize) -> bool, rng: &mut TestRng) -> Option<(usize, usize)> {
        let pairs: Vec<(usize, usize)> = (0..count)
            .flat_map(|first| (first + 1..count).map(move |second| (first, second)))
            .filter(|&(first, second)| targeted(first) || targeted(second))
            .collect();

        (!pairs.is_empty()).then(|| pairs[rng.random_range(0..pairs.len())])
    }

    /// An asset of the transaction other than `except`, or a fresh one.
    fn draw_asset(transaction: &FinalTransaction, except: Option<AssetId>, rng: &mut TestRng) -> AssetId {
        let others: Vec<AssetId> = Self::assets(transaction)
            .into_iter()
            .filter(|asset| Some(*asset) != except)
            .collect();

        if others.is_empty() || rng.random_range(0..100) < FRESH_ASSET_PERCENT {
            AssetId::from_byte_array(rng.random())
        } else {
            others[rng.random_range(0..others.len())]
        }
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
    fn transfers(&self, transaction: &FinalTransaction, map: &UtxoMap) -> Vec<(usize, usize)> {
        let outputs = transaction.outputs();

        (0..outputs.len())
            .flat_map(|from| (0..outputs.len()).map(move |to| (from, to)))
            .filter(|&(from, to)| {
                from != to
                    && outputs[from].asset == outputs[to].asset
                    && outputs[from].amount > 0
                    && (self.targets_output(from, map) || self.targets_output(to, map))
            })
            .collect()
    }

    /// Targeted outputs with something to pay.
    fn funded_outputs(&self, transaction: &FinalTransaction, map: &UtxoMap) -> Vec<usize> {
        (0..transaction.n_outputs())
            .filter(|position| self.targets_output(*position, map))
            .filter(|position| transaction.outputs()[*position].amount > 0)
            .collect()
    }

    fn targets_output(&self, position: usize, map: &UtxoMap) -> bool {
        self.mutations.everywhere
            || map
                .old_output_index(position)
                .is_some_and(|template| self.target_outputs.contains(&template))
    }

    fn targets_input(&self, position: usize, map: &UtxoMap) -> bool {
        self.mutations.everywhere
            || map
                .old_input_index(position)
                .is_some_and(|template| self.target_inputs.contains(&template))
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
    fn find_target_outputs(transaction: &FinalTransaction, program_scenarios: &[FuzzProgramScenario]) -> Vec<usize> {
        (0..transaction.n_outputs())
            .filter(|index| {
                transaction.outputs()[*index].script_pubkey.is_empty()
                    || program_scenarios
                        .iter()
                        .any(|program_scenario| program_scenario.locks(*index))
            })
            .collect()
    }

    /// Inputs a program spends.
    fn find_target_inputs(transaction: &FinalTransaction, program_scenarios: &[FuzzProgramScenario]) -> Vec<usize> {
        (0..transaction.n_inputs())
            .filter(|index| {
                program_scenarios
                    .iter()
                    .any(|program_scenario| program_scenario.spends(*index))
            })
            .collect()
    }

    /// Program literals that fit a `u64`.
    fn literals(program_scenarios: &[FuzzProgramScenario]) -> Vec<u64> {
        program_scenarios
            .iter()
            .flat_map(FuzzProgramScenario::literals)
            .filter_map(|literal| Word::from(*literal).to_u64())
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
