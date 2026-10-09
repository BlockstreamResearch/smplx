use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Result};
use std::marker::PhantomData;

use simplicityhl::{Arguments, ResolvedType, Value, WitnessNameToValueMap, WitnessValues};

use proptest::prelude::Rng;
use proptest::prelude::Strategy;
use proptest::strategy::{NewTree, ValueTree};
use proptest::test_runner::{TestRng, TestRunner};

use smplx_sdk::program::{RandomArguments, RandomWitness};

pub struct RandomValuePool<Args, Wit> {
    phantom_data: PhantomData<(Args, Wit)>,
    _value_pool: ValuePool,
}

impl<Args, Wit> Default for RandomValuePool<Args, Wit> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData,
            _value_pool: ValuePool::default(),
        }
    }
}

impl<T, E> Debug for RandomValuePool<T, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        writeln!(f, "RandomValuePool trees...")
    }
}

pub struct ValuePoolValueTree<T> {
    current: T,
    value_pool: ValuePool,
    rng: TestRng,
    cnt: usize,
    max_bound: usize,
}

impl<T> ValuePoolValueTree<T> {
    pub fn check_utilization(&self) -> bool {
        self.cnt < self.max_bound
    }
}

impl ValueTree for ValuePoolValueTree<(Arguments, WitnessValues)> {
    type Value = (Arguments, WitnessValues);

    fn current(&self) -> Self::Value {
        self.current.clone()
    }

    fn simplify(&mut self) -> bool {
        let modified_witness = self
            .value_pool
            .probabilistically_replace(self.current.1.clone(), &mut self.rng);

        self.current.1 = modified_witness;
        self.cnt += 1;
        self.check_utilization()
    }

    fn complicate(&mut self) -> bool {
        self.simplify()
    }
}

impl<Args: RandomArguments + Debug, Wit: RandomWitness + Debug> Strategy for RandomValuePool<Args, Wit> {
    type Tree = ValuePoolValueTree<(Arguments, WitnessValues)>;
    type Value = (Arguments, WitnessValues);

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let args = Args::generate_arguments(runner.rng());
        let wit = Wit::generate_witness(runner.rng());

        let pool = ValuePool::new(&wit.clone(), &args.clone());
        let wit = pool.probabilistically_replace(wit, runner.rng());

        Ok(ValuePoolValueTree {
            current: (args, wit),
            value_pool: pool,
            rng: runner.rng().clone(),
            cnt: 0,
            max_bound: 50,
        })
    }
}

#[derive(Default)]
pub struct ValuePool {
    pool: HashMap<ResolvedType, Vec<Value>>,
}

impl ValuePool {
    pub fn new(wit: &WitnessValues, args: &Arguments) -> Self {
        let mut pool: HashMap<ResolvedType, Vec<Value>> = HashMap::new();

        let mut wit_entries: Vec<_> = wit.iter().collect();
        wit_entries.sort_unstable_by_key(|(name, _)| *name);

        for (_, val) in wit_entries {
            pool.entry(val.ty().clone())
                .and_modify(|counter| counter.push(val.clone()))
                .or_insert(vec![val.clone()]);
        }

        let mut args_entries: Vec<_> = args.iter().collect();
        args_entries.sort_unstable_by_key(|(name, _)| *name);

        for (_, val) in args_entries {
            pool.entry(val.ty().clone())
                .and_modify(|counter| counter.push(val.clone()))
                .or_insert(vec![val.clone()]);
        }

        Self { pool }
    }

    pub fn sample(&self, ty: &ResolvedType, rng: &mut TestRng) -> Option<Value> {
        self.pool.get(ty).and_then(|values| {
            if values.is_empty() {
                None
            } else {
                let idx = rng.random_range(0..values.len());
                Some(values[idx].clone())
            }
        })
    }

    pub fn probabilistically_replace(&self, wit: WitnessValues, rng: &mut TestRng) -> WitnessValues {
        let mut map = HashMap::new();

        let mut entries: Vec<_> = wit.iter().collect();
        entries.sort_unstable_by_key(|(name, _)| *name);

        for (name, val) in entries {
            let should_replace: bool = rng.random();

            if should_replace {
                let sampled = self.sample(val.ty(), rng).unwrap_or_else(|| val.clone());
                map.insert(name.clone(), sampled);
            } else {
                map.insert(name.clone(), val.clone());
            }
        }

        WitnessValues::from_map(map)
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

    use crate::fuzz::args_strategy::RandomValuePool;

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

        let strategy = RandomValuePool::<Fields, Fields>::default();

        let mut runner_1 = deterministic_runner();
        let mut runner_2 = deterministic_runner();

        for case in 0..ITERATIONS {
            let mut first = strategy.new_tree(&mut runner_1).unwrap();
            let mut second = strategy.new_tree(&mut runner_2).unwrap();

            assert_eq!(
                first.current(),
                second.current(),
                "same seed produced a different pool case {case}"
            );

            for step in 0..ITERATIONS {
                let (first_changed, second_changed) = if step % 2 == 0 {
                    (first.simplify(), second.simplify())
                } else {
                    (first.complicate(), second.complicate())
                };

                assert_eq!(
                    first_changed, second_changed,
                    "pool case {case} diverged at step {step}"
                );
                assert_eq!(
                    first.current(),
                    second.current(),
                    "pool case {case} diverged at step {step}"
                );
            }
        }
    }
}
