use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

use simplicityhl::{Arguments, WitnessNameToValueMap, WitnessValues};

use proptest::prelude::Strategy;
use proptest::strategy::{NewTree, ValueTree};
use proptest::test_runner::TestRunner;

use smplx_sdk::program::{RandomArguments, RandomWitness};

use crate::fuzz::utils::generate_interesting_or_scratch_by_ty;

pub struct InterestingRandom<Args, Wit> {
    phantom_data: PhantomData<(Args, Wit)>,
}

impl<Args, Wit> Default for InterestingRandom<Args, Wit> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData,
        }
    }
}

impl<T, E> Debug for InterestingRandom<T, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "InterestingRandom trees...")
    }
}

pub struct InterestingRandomValueTree<T>(T);

impl<T: Clone + std::fmt::Debug> ValueTree for InterestingRandomValueTree<T> {
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

impl<Args: RandomArguments + std::fmt::Debug, Wit: RandomWitness + std::fmt::Debug> Strategy
    for InterestingRandom<Args, Wit>
{
    type Tree = InterestingRandomValueTree<(Arguments, WitnessValues)>;
    type Value = (Arguments, WitnessValues);

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let args = Args::generate_arguments(runner.rng());
        let wit = Wit::generate_witness(runner.rng());

        let mut args_map = HashMap::new();
        for (name, val) in args.iter() {
            args_map.insert(
                name.clone(),
                generate_interesting_or_scratch_by_ty(val.ty(), runner.rng()),
            );
        }

        let mut wit_map = HashMap::new();
        for (name, val) in wit.iter() {
            wit_map.insert(
                name.clone(),
                generate_interesting_or_scratch_by_ty(val.ty(), runner.rng()),
            );
        }

        Ok(InterestingRandomValueTree((
            Arguments::from_map(args_map),
            WitnessValues::from_map(wit_map),
        )))
    }
}
