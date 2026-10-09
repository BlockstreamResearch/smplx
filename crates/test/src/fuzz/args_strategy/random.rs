use std::fmt::{Debug, Formatter, Result};
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
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        writeln!(f, "Random trees...")
    }
}

pub struct RandomValueTree<T>(T);

impl<T: Clone + Debug> ValueTree for RandomValueTree<T> {
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

impl<Args: RandomArguments + Debug, Wit: RandomWitness + Debug> Strategy for Random<Args, Wit> {
    type Tree = RandomValueTree<(Arguments, WitnessValues)>;
    type Value = (Arguments, WitnessValues);

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        Ok(RandomValueTree((
            Args::generate_arguments(runner.rng()),
            Wit::generate_witness(runner.rng()),
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use proptest::prelude::{RngCore, Strategy};
    use proptest::strategy::ValueTree;
    use proptest::test_runner::{Config, RngSeed, TestRunner};

    use simplicityhl::num::{NonZeroPow2Usize, U256};
    use simplicityhl::types::TypeConstructible;
    use simplicityhl::value::ValueConstructible;
    use simplicityhl::{Arguments, ResolvedType, TemplateProgramWitness, Value, WitnessNameToValueMap, WitnessValues};

    use smplx_sdk::program::{RandomArguments, RandomWitness};

    use crate::fuzz::args_strategy::Random;

    #[derive(Clone, Debug)]
    struct Fields;

    fn get_fields() -> HashMap<TemplateProgramWitness, Value> {
        HashMap::from([
            (TemplateProgramWitness::parameter_from_str("U1"), Value::u1(1)),
            (TemplateProgramWitness::parameter_from_str("U8"), Value::u8(2)),
            (TemplateProgramWitness::parameter_from_str("U16"), Value::u16(3)),
            (TemplateProgramWitness::parameter_from_str("U32"), Value::u32(4)),
            (TemplateProgramWitness::parameter_from_str("U64"), Value::u64(4)),
            (TemplateProgramWitness::parameter_from_str("U128"), Value::u128(4)),
            (
                TemplateProgramWitness::parameter_from_str("U256"),
                Value::u256(U256::from_byte_array(Default::default())),
            ),
            (TemplateProgramWitness::parameter_from_str("U2"), Value::u2(0)),
            (TemplateProgramWitness::parameter_from_str("U4"), Value::u4(0)),
            (
                TemplateProgramWitness::parameter_from_str("BOOL_FALSE"),
                Value::from(false),
            ),
            (
                TemplateProgramWitness::parameter_from_str("BOOL_TRUE"),
                Value::from(true),
            ),
            (TemplateProgramWitness::parameter_from_str("UNIT"), Value::unit()),
            (
                TemplateProgramWitness::parameter_from_str("EITHER_LEFT"),
                Value::left(Value::u8(0), ResolvedType::u16()),
            ),
            (
                TemplateProgramWitness::parameter_from_str("EITHER_RIGHT"),
                Value::right(ResolvedType::u8(), Value::u16(0)),
            ),
            (
                TemplateProgramWitness::parameter_from_str("OPTION_NONE"),
                Value::none(ResolvedType::u8()),
            ),
            (
                TemplateProgramWitness::parameter_from_str("OPTION_SOME"),
                Value::some(Value::u8(0)),
            ),
            (
                TemplateProgramWitness::parameter_from_str("PRODUCT"),
                Value::product(Value::u8(0), Value::from(false)),
            ),
            (
                TemplateProgramWitness::parameter_from_str("TUPLE"),
                Value::tuple([Value::u8(0), Value::u16(0), Value::from(false)]),
            ),
            (
                TemplateProgramWitness::parameter_from_str("ARRAY_EMPTY"),
                Value::array([], ResolvedType::u8()),
            ),
            (
                TemplateProgramWitness::parameter_from_str("ARRAY"),
                Value::array([Value::u8(0), Value::u8(0)], ResolvedType::u8()),
            ),
            (
                TemplateProgramWitness::parameter_from_str("LIST_EMPTY"),
                Value::list([], ResolvedType::u8(), NonZeroPow2Usize::TWO),
            ),
            (
                TemplateProgramWitness::parameter_from_str("LIST_NONEMPTY"),
                Value::list([Value::u8(0)], ResolvedType::u8(), NonZeroPow2Usize::TWO),
            ),
        ])
    }

    impl From<Fields> for Arguments {
        fn from(_: Fields) -> Self {
            Arguments::from_map(get_fields())
        }
    }

    impl From<Fields> for WitnessValues {
        fn from(_: Fields) -> Self {
            WitnessValues::from_map(get_fields())
        }
    }

    impl RandomArguments for Fields {
        fn generate_arguments(_: &mut dyn RngCore) -> Arguments {
            Fields.into()
        }
    }

    impl RandomWitness for Fields {
        fn generate_witness(_: &mut dyn RngCore) -> WitnessValues {
            Fields.into()
        }
    }

    fn deterministic_runner() -> TestRunner {
        let config = Config {
            rng_seed: RngSeed::Fixed(0x0000_0734),
            failure_persistence: None,
            ..Config::default()
        };

        TestRunner::new(config)
    }

    #[test]
    fn strategy_is_persistent() {
        const ITERATIONS: usize = 1024;

        let strategy = Random::<Fields, Fields>::default();

        let mut runner_1 = deterministic_runner();
        let mut runner_2 = deterministic_runner();

        let generate = |runner: &mut TestRunner| strategy.new_tree(runner).unwrap().current();

        for _ in 0..ITERATIONS {
            let first = generate(&mut runner_1);
            let second = generate(&mut runner_2);

            assert_eq!(first, second, "same seed produced a different random case");
        }
    }
}
