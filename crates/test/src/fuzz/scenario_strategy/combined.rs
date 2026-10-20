use std::borrow::Cow;

use proptest::strategy::{NewTree, Strategy, ValueTree};
use proptest::test_runner::{Reason, TestRng, TestRunner};

use rand::RngExt;

use smplx_sdk::transaction::FinalTransaction;

use crate::fuzz::common::FixedValueTree;
use crate::fuzz::mutations::{Mutations, Mutator};
use crate::fuzz::outcome::SetupError;
use crate::fuzz::program::FuzzProgramSchema;
use crate::fuzz::program_scenario::FuzzProgramScenario;
use crate::fuzz::program_strategy::{FuzzProgramStrategy, FuzzProgramValues, GuidedCase, RandomCase};
use crate::fuzz::scenario_strategy::{CaseChange, CaseValues};
use crate::fuzz::transaction::FuzzTransactionSchema;

const VIOLATION_PERCENT: u32 = 50;
const MUTATION_PERCENT: u32 = 50;

/// Combines the strategies of every program of a case. Guided programs are generated together, so they share literals and values.
#[derive(Debug)]
pub(crate) struct CombinedStrategy {
    strategies: Vec<FuzzProgramStrategy>,
    transaction: FuzzTransactionSchema,
    requirements: bool,
    mutator: Option<Mutator>,
}

impl CombinedStrategy {
    /// # Errors
    /// Returns a [`SetupError`] when `mutations` cannot mutate `transaction`.
    pub(crate) fn new(
        program_scenarios: &[FuzzProgramScenario],
        transaction: &FinalTransaction,
        mutations: Option<&Mutations>,
    ) -> Result<Self, SetupError> {
        let mutator = mutations
            .map(|mutations| Mutator::new(mutations.clone(), transaction, program_scenarios))
            .transpose()?;

        Ok(Self {
            strategies: program_scenarios
                .iter()
                .map(FuzzProgramScenario::configured_strategy)
                .collect(),
            transaction: FuzzTransactionSchema::of(transaction),
            requirements: program_scenarios.iter().any(FuzzProgramScenario::has_requirements),
            mutator,
        })
    }

    /// The case's change, with the transaction schema to generate program values against.
    fn draw_change(&self, rng: &mut TestRng) -> (CaseChange, Cow<'_, FuzzTransactionSchema>) {
        if self.requirements && rng.random_range(0..100) < VIOLATION_PERCENT {
            return (CaseChange::Violation(rng.random()), Cow::Borrowed(&self.transaction));
        }

        match &self.mutator {
            Some(mutator) if rng.random_range(0..100) < MUTATION_PERCENT => {
                let (mutations, transaction) = mutator.generate(rng);

                (
                    CaseChange::Mutations(mutations),
                    Cow::Owned(FuzzTransactionSchema::of(&transaction)),
                )
            }
            _ => (CaseChange::None, Cow::Borrowed(&self.transaction)),
        }
    }

    fn generate_programs(
        &self,
        runner: &mut TestRunner,
        transaction: &FuzzTransactionSchema,
    ) -> Result<Vec<FuzzProgramValues>, Reason> {
        let schemas: Vec<&FuzzProgramSchema> = self
            .strategies
            .iter()
            .filter_map(|strategy| match strategy {
                FuzzProgramStrategy::Guided(schema) => Some(schema),
                FuzzProgramStrategy::Random(_) | FuzzProgramStrategy::Other(_) => None,
            })
            .collect();

        let mut guided = GuidedCase::new(&schemas, runner.rng())
            .with_transaction_schema(transaction)
            .generate()
            .into_iter();
        let mut values = Vec::with_capacity(self.strategies.len());

        for strategy in &self.strategies {
            values.push(match strategy {
                FuzzProgramStrategy::Guided(_) => guided.next().expect("one value per guided program"),
                FuzzProgramStrategy::Random(schema) => RandomCase::new(schema, runner.rng()).generate(),
                FuzzProgramStrategy::Other(strategy) => strategy.new_tree(runner)?.current(),
            });
        }

        Ok(values)
    }
}

impl Strategy for CombinedStrategy {
    type Tree = FixedValueTree<CaseValues>;
    type Value = CaseValues;

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let (change, transaction) = self.draw_change(runner.rng());
        let programs = self.generate_programs(runner, &transaction)?;

        Ok(FixedValueTree(CaseValues { programs, change }))
    }
}
