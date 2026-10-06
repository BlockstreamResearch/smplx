use std::marker::PhantomData;

use proptest::prelude::{BoxedStrategy, Strategy};

use simplicityhl::{Arguments, WitnessValues};

use crate::fuzz::args_strategy::{InterestingRandom, Random, RandomValuePool};

pub struct ArgsStrategyBuilder<Args, Wit, BaseStrat = InterestingRandom<Args, Wit>> {
    base_strat: BaseStrat,
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
            base_strat: InterestingRandom::default(),
            _placeholder: Default::default(),
        }
    }
}

impl<Args, Wit, BaseStrat> ArgsStrategyBuilder<Args, Wit, BaseStrat> {
    pub fn with_random(self) -> ArgsStrategyBuilder<Args, Wit, Random<Args, Wit>> {
        ArgsStrategyBuilder {
            base_strat: Random::<Args, Wit>::default(),
            _placeholder: Default::default(),
        }
    }

    pub fn with_random_pool(self) -> ArgsStrategyBuilder<Args, Wit, RandomValuePool<Args, Wit>> {
        ArgsStrategyBuilder {
            base_strat: RandomValuePool::<Args, Wit>::default(),
            _placeholder: Default::default(),
        }
    }

    pub fn with_custom_strategy<NewStrat>(self, custom_strat: NewStrat) -> ArgsStrategyBuilder<Args, Wit, NewStrat>
    where
        NewStrat: Strategy<Value = (Arguments, WitnessValues)> + 'static,
    {
        ArgsStrategyBuilder {
            base_strat: custom_strat,
            _placeholder: Default::default(),
        }
    }

    pub fn with_random_interesting_values(self) -> ArgsStrategyBuilder<Args, Wit, InterestingRandom<Args, Wit>> {
        ArgsStrategyBuilder {
            base_strat: InterestingRandom::<Args, Wit>::default(),
            _placeholder: Default::default(),
        }
    }
}

impl<Args, Wit, BaseStrat> ArgsStrategyBuilder<Args, Wit, BaseStrat>
where
    BaseStrat: Strategy<Value = (Arguments, WitnessValues)> + 'static,
{
    pub fn build(self) -> BoxedStrategy<(Arguments, WitnessValues)> {
        self.base_strat.boxed()
    }
}
