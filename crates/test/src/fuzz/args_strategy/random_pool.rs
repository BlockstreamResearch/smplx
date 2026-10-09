use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Result};
use std::marker::PhantomData;
use std::sync::Arc;

use simplicityhl::{Arguments, ResolvedType, Value, WitnessNameToValueMap, WitnessValues};

use proptest::prelude::{Rng, Strategy};
use proptest::strategy::{NewTree, ValueTree};
use proptest::test_runner::{TestRng, TestRunner};

use smplx_sdk::program::{ConstProvider, RandomArguments, RandomWitness};

use crate::fuzz::generate_interesting_or_scratch_by_ty;

pub struct RandomValuePool<Args, Wit, Const = NoConst> {
    phantom_data: PhantomData<(Args, Wit, Const)>,
    constants: Arc<HashMap<ResolvedType, Vec<Value>>>,
}

pub struct NoConst {}

impl ConstProvider for NoConst {
    fn get_constants() -> &'static [Value] {
        &[]
    }
}

impl<Args, Wit, Const> Default for RandomValuePool<Args, Wit, Const> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData,
            constants: Arc::default(),
        }
    }
}

impl<Args, Wit> RandomValuePool<Args, Wit> {
    pub fn with_constants(constants: &[Value]) -> Self {
        let mut map: HashMap<ResolvedType, Vec<Value>> = Default::default();
        ValuePool::insert_values_inner(&mut map, constants);

        Self {
            phantom_data: PhantomData,
            constants: Arc::new(map),
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

        let pool = ValuePool::new()
            .insert(&args)
            .insert(&wit)
            .with_shared_constants(Arc::clone(&self.constants));
        let wit = pool.probabilistically_replace(wit, runner.rng());

        let max_bound = pool.pool.len();
        Ok(ValuePoolValueTree {
            current: (args, wit),
            value_pool: pool,
            rng: runner.rng().clone(),
            cnt: 0,
            max_bound,
        })
    }
}

trait PoolValues {
    fn for_each_value(&self, insert: impl FnMut(&Value));
}

impl PoolValues for Arguments {
    fn for_each_value(&self, mut insert: impl FnMut(&Value)) {
        let mut args_entries: Vec<_> = self.iter().collect();
        args_entries.sort_unstable_by_key(|(name, _)| *name);

        for (_, value) in args_entries {
            insert(value);
        }
    }
}

impl PoolValues for WitnessValues {
    fn for_each_value(&self, mut insert: impl FnMut(&Value)) {
        let mut wit_entries: Vec<_> = self.iter().collect();
        wit_entries.sort_unstable_by_key(|(name, _)| *name);

        for (_, value) in wit_entries {
            insert(value);
        }
    }
}

impl PoolValues for [Value] {
    fn for_each_value(&self, mut insert: impl FnMut(&Value)) {
        // we don't require sorting, as our values are collected via contract traversal
        for value in self {
            insert(value);
        }
    }
}

#[derive(Default)]
pub struct ValuePool {
    pool: HashMap<ResolvedType, Vec<Value>>,
    constants: Arc<HashMap<ResolvedType, Vec<Value>>>,
}

impl ValuePool {
    pub fn new() -> Self {
        Self::default()
    }

    fn insert(mut self, values: &(impl PoolValues + ?Sized)) -> Self {
        Self::insert_values_inner(&mut self.pool, values);
        self
    }

    fn with_shared_constants(mut self, constants: Arc<HashMap<ResolvedType, Vec<Value>>>) -> Self {
        self.constants = constants;
        self
    }

    pub fn sample(&self, ty: &ResolvedType, rng: &mut TestRng) -> Option<Value> {
        match rng.random::<bool>() {
            true => {
                let local_pool = self.pool.get(ty).map(Vec::as_slice).unwrap_or_default();
                let constants = self.constants.get(ty).map(Vec::as_slice).unwrap_or_default();

                let local_pool_len = local_pool.len();
                let count = local_pool_len + constants.len();
                if count == 0 {
                    return None;
                }

                let index = rng.random_range(0..count);
                if index < local_pool_len {
                    Some(local_pool[index].clone())
                } else {
                    Some(constants[index - local_pool_len].clone())
                }
            }
            false => Some(generate_interesting_or_scratch_by_ty(ty, rng)),
        }
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

    fn insert_values_inner(map: &mut HashMap<ResolvedType, Vec<Value>>, values: &(impl PoolValues + ?Sized)) {
        values.for_each_value(|value| {
            map.entry(value.ty().clone()).or_default().push(value.clone());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use proptest::prelude::{RngCore, Strategy};
    use proptest::strategy::ValueTree;
    use proptest::test_runner::{Config, RngAlgorithm};
    use proptest::test_runner::{RngSeed, TestRunner};

    use simplicityhl::TemplateProgramWitness;
    use simplicityhl::num::{NonZeroPow2Usize, U256};
    use simplicityhl::types::TypeConstructible;
    use simplicityhl::value::ValueConstructible;
    use simplicityhl::{Arguments, ResolvedType, Value, WitnessNameToValueMap, WitnessValues};

    use smplx_sdk::program::{RandomArguments, RandomWitness};

    use crate::fuzz::args_strategy::RandomValuePool;

    fn deterministic_rng() -> TestRng {
        TestRng::from_seed(RngAlgorithm::ChaCha, &[17; 32])
    }

    fn deterministic_runner() -> TestRunner {
        let config = Config {
            rng_seed: RngSeed::Fixed(0x0000_0734),
            failure_persistence: None,
            ..Config::default()
        };

        TestRunner::new(config)
    }

    fn arguments(value: Value) -> Arguments {
        Arguments::from_map(HashMap::from([(
            TemplateProgramWitness::parameter_from_str("ARG"),
            value,
        )]))
    }

    fn witness(value: Value) -> WitnessValues {
        WitnessValues::from_map(HashMap::from([(
            TemplateProgramWitness::witness_from_str("WIT"),
            value,
        )]))
    }

    #[derive(Debug)]
    struct GeneratedArgs;

    impl From<GeneratedArgs> for Arguments {
        fn from(_: GeneratedArgs) -> Self {
            arguments(Value::u16(1))
        }
    }

    impl RandomArguments for GeneratedArgs {
        fn generate_arguments(rng: &mut dyn rand::RngCore) -> Arguments {
            arguments(Value::u16(rng.next_u32() as u16))
        }
    }

    #[derive(Debug)]
    struct GeneratedWitness;

    impl From<GeneratedWitness> for WitnessValues {
        fn from(_: GeneratedWitness) -> Self {
            witness(Value::u16(2))
        }
    }

    impl RandomWitness for GeneratedWitness {
        fn generate_witness(_: &mut dyn rand::RngCore) -> WitnessValues {
            witness(Value::u16(2))
        }
    }

    #[derive(Clone, Debug)]
    struct Fields;

    impl From<Fields> for Arguments {
        fn from(_: Fields) -> Self {
            Arguments::from_map(Fields::get_fields())
        }
    }

    impl From<Fields> for WitnessValues {
        fn from(_: Fields) -> Self {
            WitnessValues::from_map(Fields::get_fields())
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

    impl Fields {
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
    }

    #[test]
    fn samples_local_values_and_matching_constants() {
        let constant = Value::u16(1337);
        let _unused_type = Value::u8(99);

        let strategy =
            RandomValuePool::<GeneratedArgs, GeneratedWitness>::with_constants(&[constant.clone(), _unused_type]);

        let args_value = Value::u16(1);

        let pool = ValuePool::new()
            .insert(&arguments(args_value.clone()))
            .with_shared_constants(Arc::clone(&strategy.constants));
        let mut rng = deterministic_rng();

        let mut saw_local = false;
        let mut saw_constant = false;

        // Assert that both the constant and argument values are sampled
        for _ in 0..64 {
            let value = pool.sample(constant.ty(), &mut rng).unwrap();
            assert_eq!(value.ty(), constant.ty());
            saw_local |= value == args_value;
            saw_constant |= value == constant;
        }
        assert!(saw_local && saw_constant);

        // Assert that the constant value is used as a replacement
        let mut replaced = false;
        let wit_value = Value::u16(2);
        for _ in 0..64 {
            let output = pool.probabilistically_replace(witness(wit_value.clone()), &mut rng);
            let value = output.get(&TemplateProgramWitness::witness_from_str("WIT")).unwrap();
            assert_eq!(value.ty(), constant.ty());
            replaced |= value == &constant;
        }
        assert!(replaced);
    }

    #[test]
    fn trees_share_constants_and_keep_their_input_pools_independent() {
        let constant = Value::u16(1337);
        let strategy = RandomValuePool::<GeneratedArgs, GeneratedWitness>::with_constants(&[constant]);
        let mut runner = TestRunner::new_with_rng(Config::default(), deterministic_rng());
        let first = strategy.new_tree(&mut runner).unwrap();
        let second = strategy.new_tree(&mut runner).unwrap();

        // Both trees share the strategy's constants
        assert!(Arc::ptr_eq(&strategy.constants, &first.value_pool.constants));
        assert!(Arc::ptr_eq(&first.value_pool.constants, &second.value_pool.constants));

        // Both local pools contain the same number of values
        assert_eq!(strategy.constants.values().map(Vec::len).sum::<usize>(), 1);
        assert_eq!(first.value_pool.pool.values().map(Vec::len).sum::<usize>(), 2);
        assert_eq!(second.value_pool.pool.values().map(Vec::len).sum::<usize>(), 2);

        // The trees contain different generated values
        assert_ne!(first.current.0, second.current.0);
        assert_ne!(first.value_pool.pool, second.value_pool.pool);
    }

    #[test]
    fn default_strategy_uses_inputs_without_constants() {
        let strategy = RandomValuePool::<GeneratedArgs, GeneratedWitness>::default();
        let mut runner = TestRunner::new_with_rng(Config::default(), deterministic_rng());
        let tree = strategy.new_tree(&mut runner).unwrap();

        // The tree has no constants when the strategy is initialized without them
        assert!(tree.value_pool.constants.is_empty());
        assert_eq!(tree.value_pool.pool.values().map(Vec::len).sum::<usize>(), 2);
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
