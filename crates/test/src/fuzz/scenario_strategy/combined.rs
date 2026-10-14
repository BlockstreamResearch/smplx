use proptest::strategy::{NewTree, Strategy, ValueTree};
use proptest::test_runner::TestRunner;

use simplicityhl::{Arguments, WitnessValues};

use crate::fuzz::common::FixedValueTree;
use crate::fuzz::program_strategy::{GuidedCase, GuidedSchema, ProgramStrategy};

type Values = (Arguments, WitnessValues);

/// Combines the strategies of every program of a case. Guided programs are generated together, so they share literals and values.
#[derive(Debug)]
pub(crate) struct Combined(Vec<ProgramStrategy>);

impl Combined {
    pub(crate) fn new(strategies: Vec<ProgramStrategy>) -> Self {
        Self(strategies)
    }
}

impl Strategy for Combined {
    type Tree = FixedValueTree<Vec<Values>>;
    type Value = Vec<Values>;

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let schemas: Vec<&GuidedSchema> = self
            .0
            .iter()
            .filter_map(|strategy| match strategy {
                ProgramStrategy::Guided(schema) => Some(schema),
                ProgramStrategy::Random(_) | ProgramStrategy::Other(_) => None,
            })
            .collect();

        let mut guided = GuidedCase::new(&schemas, runner.rng()).generate().into_iter();
        let mut values = Vec::with_capacity(self.0.len());

        for strategy in &self.0 {
            values.push(match strategy {
                ProgramStrategy::Guided(_) => guided.next().expect("one value per guided program"),
                ProgramStrategy::Random(strategy) | ProgramStrategy::Other(strategy) => {
                    strategy.new_tree(runner)?.current()
                }
            });
        }

        Ok(FixedValueTree(values))
    }
}
