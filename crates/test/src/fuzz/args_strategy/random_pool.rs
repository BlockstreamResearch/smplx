use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Result};
use std::marker::PhantomData;
use std::sync::Arc;

use simplicityhl::{Arguments, ResolvedType, Value, WitnessNameToValueMap, WitnessValues};

use proptest::prelude::Rng;
use proptest::prelude::Strategy;
use proptest::strategy::{NewTree, ValueTree};
use proptest::test_runner::{TestRng, TestRunner};

use smplx_sdk::program::{RandomArguments, RandomWitness};

pub struct RandomValuePool<Args, Wit> {
    phantom_data: PhantomData<(Args, Wit)>,
    constants: Arc<HashMap<ResolvedType, Vec<Value>>>,
}

impl<Args, Wit> Default for RandomValuePool<Args, Wit> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData,
            constants: Arc::default(),
        }
    }
}

impl<Args, Wit> RandomValuePool<Args, Wit> {
    pub fn with_constants(constants: &[(ResolvedType, Value)]) -> Self {
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

        Ok(ValuePoolValueTree {
            current: (args, wit),
            value_pool: pool,
            rng: runner.rng().clone(),
            cnt: 0,
            max_bound: 50,
        })
    }
}

trait PoolValues {
    fn for_each_value(&self, insert: impl FnMut(&Value));
}

impl PoolValues for Arguments {
    fn for_each_value(&self, mut insert: impl FnMut(&Value)) {
        for (_, value) in self.iter() {
            insert(value);
        }
    }
}

impl PoolValues for WitnessValues {
    fn for_each_value(&self, mut insert: impl FnMut(&Value)) {
        for (_, value) in self.iter() {
            insert(value);
        }
    }
}

impl PoolValues for [(ResolvedType, Value)] {
    fn for_each_value(&self, mut insert: impl FnMut(&Value)) {
        for (_, value) in self {
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

    pub fn probabilistically_replace(&self, wit: WitnessValues, rng: &mut TestRng) -> WitnessValues {
        let mut map = HashMap::new();

        for (name, val) in wit.iter() {
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
    use proptest::test_runner::{Config, RngAlgorithm};
    use simplicityhl::TemplateProgramWitness;
    use simplicityhl::value::ValueConstructible;

    fn deterministic_rng() -> TestRng {
        TestRng::from_seed(RngAlgorithm::ChaCha, &[17; 32])
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

    #[test]
    fn all_sources_insert_values_under_their_types() {
        let args = arguments(Value::u8(7));
        let wit = witness(Value::u16(42));

        let constant = Value::u32(1337);
        let constants = [(constant.ty().clone(), constant.clone())];

        let pool = ValuePool::new().insert(&args).insert(&wit).insert(constants.as_slice());
        let mut rng = deterministic_rng();

        for value in [Value::u8(7), Value::u16(42), constant] {
            assert_eq!(pool.sample(value.ty(), &mut rng), Some(value));
        }
        assert_eq!(pool.sample(Value::u64(0).ty(), &mut rng), None);
    }

    #[test]
    fn samples_local_values_and_matching_constants() {
        let constant = Value::u16(1337);
        let _unused_type = Value::u8(99);

        let strategy = RandomValuePool::<GeneratedArgs, GeneratedWitness>::with_constants(&[
            (constant.ty().clone(), constant.clone()),
            (_unused_type.ty().clone(), _unused_type),
        ]);

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
        let strategy =
            RandomValuePool::<GeneratedArgs, GeneratedWitness>::with_constants(&[(constant.ty().clone(), constant)]);
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
}
