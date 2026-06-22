use proptest::prelude::{BoxedStrategy, Strategy};
use proptest::strategy::{NewTree, ValueTree};
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::sync::Arc;

use crate::mutantesting::core::ArgGenFuzzStrategy;
use crate::mutantesting::FuzzContext;
use proptest::prelude::Rng;
use proptest::test_runner::{TestRng, TestRunner};
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::str::WitnessName;
use simplicityhl::{Arguments, ResolvedType, Value, WitnessValues};
use smplx_sdk::program::{RandomArguments, RandomWitness};
use std::collections::HashMap;

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

pub struct RandomValueTree<T>(pub T);

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

impl<Args: RandomArguments + std::fmt::Debug + Clone + 'static, Wit: RandomWitness + std::fmt::Debug + Clone + 'static>
    ArgGenFuzzStrategy<Args, Wit> for Random<Args, Wit>
{
    fn get_strategy(&self, _test_context: Arc<FuzzContext>) -> BoxedStrategy<(Arguments, WitnessValues)> {
        Random::<Args, Wit>::default().boxed()
    }
}

pub trait PstBuilder<Args, Wit> {
    fn build_pst(&self, ctx: &FuzzContext, args: &Args, wit: &Wit) -> (PartiallySignedTransaction, Args, Wit);
}

pub struct PstGeneratorTree<Args, Wit, B> {
    inner_tree: RandomValueTree<(Args, Wit)>,
    context: Arc<FuzzContext>,
    builder: Arc<B>,
}

impl<Args: Clone + std::fmt::Debug, Wit: Clone + std::fmt::Debug, B: PstBuilder<Args, Wit>> ValueTree
    for PstGeneratorTree<Args, Wit, B>
{
    type Value = (Args, Wit, PartiallySignedTransaction);

    fn current(&self) -> Self::Value {
        let (args, wit) = self.inner_tree.current();

        // Because current() must be deterministic for the given args/wit,
        // you construct the PST deterministically here:
        let (pst, modified_args, modified_wit) = self.builder.build_pst(&self.context, &args, &wit);

        (modified_args, modified_wit, pst)
    }
    fn simplify(&mut self) -> bool {
        self.inner_tree.simplify()
    }
    fn complicate(&mut self) -> bool {
        self.inner_tree.complicate()
    }
}

pub struct PstGeneratorStrategy<Args, Wit, B> {
    pub builder: Arc<B>,
    pub context: Arc<FuzzContext>,
    phantom_data: PhantomData<(Args, Wit)>,
}

impl<Args, Wit, B> PstGeneratorStrategy<Args, Wit, B> {
    pub fn new(builder: Arc<B>, context: Arc<FuzzContext>) -> Self {
        Self {
            builder,
            context,
            phantom_data: PhantomData,
        }
    }
}

impl<Args, Wit, B> Debug for PstGeneratorStrategy<Args, Wit, B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "PstGeneratorStrategy...")
    }
}

impl<
    Args: RandomArguments + std::fmt::Debug + Clone + 'static,
    Wit: RandomWitness + std::fmt::Debug + Clone + 'static,
    B: PstBuilder<Arguments, WitnessValues> + std::fmt::Debug + Clone + 'static,
> Strategy for PstGeneratorStrategy<Args, Wit, B>
{
    type Tree = PstGeneratorTree<Arguments, WitnessValues, B>;
    type Value = (Arguments, WitnessValues, PartiallySignedTransaction);

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        Ok(PstGeneratorTree {
            inner_tree: RandomValueTree((
                Args::generate_arguments(runner.rng()),
                Wit::generate_witness(runner.rng()),
            )),
            context: self.context.clone(),
            builder: self.builder.clone(),
        })
    }
}

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

impl<Args: RandomArguments + std::fmt::Debug + Clone + 'static, Wit: RandomWitness + std::fmt::Debug + Clone + 'static>
    ArgGenFuzzStrategy<Args, Wit> for RandomValuePool<Args, Wit>
{
    fn get_strategy(&self, _test_context: Arc<FuzzContext>) -> BoxedStrategy<(Arguments, WitnessValues)> {
        RandomValuePool::<Args, Wit> {
            phantom_data: Default::default(),
            _value_pool: ValuePool::default(),
        }
        .boxed()
    }
}

impl<T, E> Debug for RandomValuePool<T, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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

impl<Args: RandomArguments + std::fmt::Debug, Wit: RandomWitness + std::fmt::Debug> Strategy
    for RandomValuePool<Args, Wit>
{
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
    witness_structure: HashMap<WitnessName, ResolvedType>,
}

impl ValuePool {
    pub fn new(wit: &WitnessValues, args: &Arguments) -> Self {
        let mut pool: HashMap<ResolvedType, Vec<Value>> = HashMap::new();
        let mut witness_structure: HashMap<WitnessName, ResolvedType> = HashMap::new();

        for (name, val) in wit.iter() {
            witness_structure.insert(name.clone(), val.ty().clone());
            pool.entry(val.ty().clone())
                .and_modify(|counter| counter.push(val.clone()))
                .or_insert(vec![val.clone()]);
        }

        for (_, val) in args.iter() {
            pool.entry(val.ty().clone())
                .and_modify(|counter| counter.push(val.clone()))
                .or_insert(vec![val.clone()]);
        }
        // TODO: add possibility in simplex to generate any kind of type

        Self {
            pool,
            witness_structure,
        }
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

    pub fn generate_witness(&self, rng: &mut TestRng) -> WitnessValues {
        let mut map = HashMap::new();
        for (name, ty) in &self.witness_structure {
            if let Some(val) = self.sample(ty, rng) {
                map.insert(name.clone(), val);
            }
        }
        WitnessValues::from(map)
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
        WitnessValues::from(map)
    }
}
