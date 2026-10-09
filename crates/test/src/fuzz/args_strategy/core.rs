use std::fmt::Debug;
use std::marker::PhantomData;

use simplicityhl::{Arguments, WitnessValues};

use proptest::prelude::{BoxedStrategy, Strategy};
use proptest::strategy::ValueTree;

use crate::fuzz::args_strategy::{Guided, Random};

pub struct ArgsStrategyBuilder<P, Base = Guided<P>> {
    base_strategy: Base,
    _program: PhantomData<fn() -> P>,
}

impl<P> ArgsStrategyBuilder<P> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<P> Default for ArgsStrategyBuilder<P> {
    fn default() -> Self {
        Self {
            base_strategy: Guided::default(),
            _program: PhantomData,
        }
    }
}

impl<P, Base> ArgsStrategyBuilder<P, Base> {
    pub fn with_random(self) -> ArgsStrategyBuilder<P, Random<P>> {
        ArgsStrategyBuilder {
            base_strategy: Random::default(),
            _program: PhantomData,
        }
    }

    pub fn with_guided(self) -> ArgsStrategyBuilder<P, Guided<P>> {
        ArgsStrategyBuilder {
            base_strategy: Guided::default(),
            _program: PhantomData,
        }
    }

    pub fn with_custom_strategy<New>(self, custom_strategy: New) -> ArgsStrategyBuilder<P, New>
    where
        New: Strategy<Value = (Arguments, WitnessValues)> + 'static,
    {
        ArgsStrategyBuilder {
            base_strategy: custom_strategy,
            _program: PhantomData,
        }
    }
}

impl<P, Base> ArgsStrategyBuilder<P, Base>
where
    Base: Strategy<Value = (Arguments, WitnessValues)> + 'static,
{
    pub fn build(self) -> BoxedStrategy<(Arguments, WitnessValues)> {
        self.base_strategy.boxed()
    }
}

/// A generated case that is never shrunk.
pub struct FixedValueTree<T>(pub T);

impl<T: Clone + Debug> ValueTree for FixedValueTree<T> {
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
