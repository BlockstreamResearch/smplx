use std::fmt::{Debug, Formatter, Result};
use std::marker::PhantomData;

use simplicityhl::{Arguments, WitnessValues};

use proptest::prelude::Strategy;
use proptest::strategy::NewTree;
use proptest::test_runner::TestRunner;

use smplx_sdk::program::ProgramSchema;

use crate::fuzz::args_strategy::FixedValueTree;
use crate::fuzz::utils::{random_arguments, random_witness};

pub struct Random<P> {
    _program: PhantomData<fn() -> P>,
}

impl<P> Default for Random<P> {
    fn default() -> Self {
        Self { _program: PhantomData }
    }
}

impl<P> Debug for Random<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        writeln!(f, "Random trees...")
    }
}

impl<P: ProgramSchema> Strategy for Random<P> {
    type Tree = FixedValueTree<(Arguments, WitnessValues)>;
    type Value = (Arguments, WitnessValues);

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let arguments = random_arguments::<P, _>(runner.rng());
        let witness = random_witness::<P, _>(runner.rng());

        Ok(FixedValueTree((arguments, witness)))
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::Strategy;
    use proptest::strategy::ValueTree;

    use crate::fuzz::args_strategy::Random;
    use crate::fuzz::args_strategy::test_schema::{RichSchema, assert_matches_schema, deterministic_runner};

    #[test]
    fn strategy_is_persistent() {
        let strategy = Random::<RichSchema>::default();

        let mut runner_1 = deterministic_runner();
        let mut runner_2 = deterministic_runner();

        for _ in 0..1024 {
            let first = strategy.new_tree(&mut runner_1).unwrap().current();
            let second = strategy.new_tree(&mut runner_2).unwrap().current();

            assert_eq!(first, second, "same seed produced a different random case");
        }
    }

    #[test]
    fn values_match_their_declared_types() {
        let strategy = Random::<RichSchema>::default();
        let mut runner = deterministic_runner();

        for _ in 0..1024 {
            let (arguments, witness) = strategy.new_tree(&mut runner).unwrap().current();

            assert_matches_schema::<RichSchema>(&arguments, &witness);
        }
    }
}
