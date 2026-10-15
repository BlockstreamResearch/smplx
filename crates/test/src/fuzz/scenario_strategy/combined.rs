use proptest::strategy::{NewTree, Strategy, ValueTree};
use proptest::test_runner::TestRunner;

use crate::fuzz::common::FixedValueTree;
use crate::fuzz::program::FuzzProgramSchema;
use crate::fuzz::program_strategy::{FuzzProgramStrategy, FuzzProgramValues, GuidedCase, RandomCase};

/// Combines the strategies of every program of a case. Guided programs are generated together, so they share literals and values.
#[derive(Debug)]
pub(crate) struct Combined(Vec<FuzzProgramStrategy>);

impl Combined {
    pub(crate) fn new(strategies: Vec<FuzzProgramStrategy>) -> Self {
        Self(strategies)
    }
}

impl Strategy for Combined {
    type Tree = FixedValueTree<Vec<FuzzProgramValues>>;
    type Value = Vec<FuzzProgramValues>;

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let schemas: Vec<&FuzzProgramSchema> = self
            .0
            .iter()
            .filter_map(|strategy| match strategy {
                FuzzProgramStrategy::Guided(schema) => Some(schema),
                FuzzProgramStrategy::Random(_) | FuzzProgramStrategy::Other(_) => None,
            })
            .collect();

        let mut guided = GuidedCase::new(&schemas, runner.rng()).generate().into_iter();
        let mut values = Vec::with_capacity(self.0.len());

        for strategy in &self.0 {
            values.push(match strategy {
                FuzzProgramStrategy::Guided(_) => guided.next().expect("one value per guided program"),
                FuzzProgramStrategy::Random(schema) => RandomCase::new(schema, runner.rng()).generate(),
                FuzzProgramStrategy::Other(strategy) => strategy.new_tree(runner)?.current(),
            });
        }

        Ok(FixedValueTree(values))
    }
}
