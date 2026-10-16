use proptest::strategy::{NewTree, Strategy, ValueTree};
use proptest::test_runner::TestRunner;

use crate::fuzz::common::FixedValueTree;
use crate::fuzz::program::FuzzProgramSchema;
use crate::fuzz::program_strategy::{FuzzProgramStrategy, FuzzProgramValues, GuidedCase, RandomCase};
use crate::fuzz::transaction::FuzzTransactionSchema;

/// Combines the strategies of every program of a case. Guided programs are generated together, so they share literals and values.
#[derive(Debug)]
pub(crate) struct Combined {
    strategies: Vec<FuzzProgramStrategy>,
    transaction: FuzzTransactionSchema,
}

impl Combined {
    pub(crate) fn new(strategies: Vec<FuzzProgramStrategy>, transaction: FuzzTransactionSchema) -> Self {
        Self {
            strategies,
            transaction,
        }
    }
}

impl Strategy for Combined {
    type Tree = FixedValueTree<Vec<FuzzProgramValues>>;
    type Value = Vec<FuzzProgramValues>;

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let schemas: Vec<&FuzzProgramSchema> = self
            .strategies
            .iter()
            .filter_map(|strategy| match strategy {
                FuzzProgramStrategy::Guided(schema) => Some(schema),
                FuzzProgramStrategy::Random(_) | FuzzProgramStrategy::Other(_) => None,
            })
            .collect();

        let mut guided = GuidedCase::new(&schemas, runner.rng())
            .with_transaction_schema(&self.transaction)
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

        Ok(FixedValueTree(values))
    }
}
