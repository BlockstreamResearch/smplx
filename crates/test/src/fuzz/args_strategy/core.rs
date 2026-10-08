use std::marker::PhantomData;

use simplicityhl::{Arguments, WitnessValues};

use proptest::prelude::{BoxedStrategy, Strategy};

use crate::fuzz::args_strategy::{InterestingRandom, Random, RandomValuePool};

pub struct ArgsStrategyBuilder<Args, Wit, Base = InterestingRandom<Args, Wit>> {
    base_strategy: Base,
    _placeholder: PhantomData<(Args, Wit)>,
}

impl<Args, Wit> ArgsStrategyBuilder<Args, Wit> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<Args, Wit> Default for ArgsStrategyBuilder<Args, Wit> {
    fn default() -> Self {
        Self {
            base_strategy: InterestingRandom::default(),
            _placeholder: Default::default(),
        }
    }
}

impl<Args, Wit, Base> ArgsStrategyBuilder<Args, Wit, Base> {
    pub fn with_random(self) -> ArgsStrategyBuilder<Args, Wit, Random<Args, Wit>> {
        ArgsStrategyBuilder {
            base_strategy: Random::<Args, Wit>::default(),
            _placeholder: Default::default(),
        }
    }

    pub fn with_random_pool(self) -> ArgsStrategyBuilder<Args, Wit, RandomValuePool<Args, Wit>> {
        ArgsStrategyBuilder {
            base_strategy: RandomValuePool::<Args, Wit>::default(),
            _placeholder: Default::default(),
        }
    }

    pub fn with_custom_strategy<New>(self, custom_strategy: New) -> ArgsStrategyBuilder<Args, Wit, New>
    where
        New: Strategy<Value = (Arguments, WitnessValues)> + 'static,
    {
        ArgsStrategyBuilder {
            base_strategy: custom_strategy,
            _placeholder: Default::default(),
        }
    }

    pub fn with_random_interesting_values(self) -> ArgsStrategyBuilder<Args, Wit, InterestingRandom<Args, Wit>> {
        ArgsStrategyBuilder {
            base_strategy: InterestingRandom::<Args, Wit>::default(),
            _placeholder: Default::default(),
        }
    }
}

impl<Args, Wit, Base> ArgsStrategyBuilder<Args, Wit, Base>
where
    Base: Strategy<Value = (Arguments, WitnessValues)> + 'static,
{
    pub fn build(self) -> BoxedStrategy<(Arguments, WitnessValues)> {
        self.base_strategy.boxed()
    }
}
