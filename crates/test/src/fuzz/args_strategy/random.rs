use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

use simplicityhl::{Arguments, WitnessValues};

use proptest::prelude::Strategy;
use proptest::strategy::{NewTree, ValueTree};
use proptest::test_runner::TestRunner;

use smplx_sdk::program::{RandomArguments, RandomWitness};

pub struct Random<Args, Wit> {
    phantom_data: PhantomData<(Args, Wit)>,
}

impl<Args, Wit> Default for Random<Args, Wit> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData,
        }
    }
}

impl<T, E> Debug for Random<T, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Random trees...")
    }
}

pub struct RandomValueTree<T>(T);

impl<T: Clone + std::fmt::Debug> ValueTree for RandomValueTree<T> {
    type Value = T;
    fn current(&self) -> T {
        self.0.clone()
    }
    fn simplify(&mut self) -> bool {
        false
    }
    fn complicate(&mut self) -> bool {
        false
    }
}

impl<Args: RandomArguments + std::fmt::Debug, Wit: RandomWitness + std::fmt::Debug> Strategy for Random<Args, Wit> {
    type Tree = RandomValueTree<(Arguments, WitnessValues)>;
    type Value = (Arguments, WitnessValues);

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        Ok(RandomValueTree((
            Args::generate_arguments(runner.rng()),
            Wit::generate_witness(runner.rng()),
        )))
    }
}
