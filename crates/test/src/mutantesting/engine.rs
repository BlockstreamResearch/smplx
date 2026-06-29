use crate::context::TestContext;
use crate::mutantesting::args_strategy::{Random, RandomValuePool};
use crate::mutantesting::blueprint_constructor::{BlueprintDraftConstructor, BlueprintFuzzStrategy};
use crate::mutantesting::core::{ContractFuzzStrategyBlueprint, FuzzContext, FuzzableProgram, SignerOption};
use crate::mutantesting::{ProgramCheck, ProgramExecResult};
use proptest::prelude::{BoxedStrategy, TestCaseError};
use proptest::strategy::Strategy;
use proptest::test_runner::TestRunner;
use simplicityhl::{Arguments, WitnessValues};
use smplx_sdk::program::{ArgumentsTrait, ProgramFactory, ProgramTrait, RandomArguments, RandomWitness, WitnessTrait};
use smplx_sdk::provider::{EsploraProvider, ProviderTrait, SimplicityNetwork};
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::FinalTransaction;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;

pub struct SimplexFuzzEngine<Program, Args, Wit> {
    runner: proptest::test_runner::TestRunner,
    fuzz_context: FuzzContext,
    local_fuzz_config: LocalFuzzConfig,
    strategy_storage: BoxedStrategy<(Arguments, WitnessValues)>,
    blueprint: Box<dyn ContractFuzzStrategyBlueprint<Program, Args, Wit>>,
}

pub struct LocalFuzzConfig {
    runs: usize,
}

pub fn get_default_provider(default_network: SimplicityNetwork) -> EsploraProvider {
    EsploraProvider::new("default_web_page.com".into(), default_network)
}

pub struct FuzzStrategyBuilder<Program, Args, Wit> {
    proptest_config: proptest::test_runner::Config,
    pub(crate) strategy_storage: Option<BoxedStrategy<(Arguments, WitnessValues)>>,
    pub(crate) blueprint: Option<Box<dyn ContractFuzzStrategyBlueprint<Program, Args, Wit>>>,
    pub(crate) signer_option: SignerOption,
    pub(crate) signer: Option<Signer>,
    test_context: Option<TestContext>,
    mock_provider: Arc<dyn ProviderTrait>,
    network: SimplicityNetwork,
    _phantom: PhantomData<(Program, Args, Wit)>,
}

impl<Program, Args, Wit> FuzzStrategyBuilder<Program, Args, Wit>
where
    Program: FuzzableProgram<Program> + ProgramFactory<Program> + Clone + 'static,
    Args: ArgumentsTrait + RandomArguments + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
{
    pub fn new(config: proptest::test_runner::Config) -> Self {
        let default_network = SimplicityNetwork::default_regtest();

        Self {
            proptest_config: config,
            strategy_storage: None,
            blueprint: None,
            test_context: None,
            signer_option: SignerOption::NoSigning,
            signer: None,
            mock_provider: Arc::new(get_default_provider(default_network)),
            network: default_network,
            _phantom: PhantomData,
        }
    }

    pub fn from_context(mut config: proptest::test_runner::Config, test_context: TestContext) -> Self {
        let default_network = SimplicityNetwork::default_regtest();
        let smplx_test_context = dbg!(test_context.get_config());
        if let Some(proptest_conf) = smplx_test_context.proptest.as_ref() {
            if let Some(cases) = proptest_conf.cases {
                config.cases = cases;
            }

            if let Some(max_global_rejects) = proptest_conf.max_global_rejects {
                config.max_global_rejects = max_global_rejects;
            }

            if let Some(max_shrink_iters) = proptest_conf.max_shrink_iters {
                config.max_shrink_iters = max_shrink_iters;
            }

            if let Some(max_local_rejects) = proptest_conf.max_local_rejects {
                config.max_local_rejects = max_local_rejects;
            }

            config.verbose = smplx_test_context.verbosity as u32;
        }

        Self {
            proptest_config: config,
            strategy_storage: None,
            blueprint: None,
            test_context: None,
            signer_option: SignerOption::DefaultTestConfigSigner,
            signer: None,
            mock_provider: Arc::new(get_default_provider(default_network)),
            network: default_network,
            _phantom: PhantomData,
        }
    }
}

impl<Program, Args, Wit> FuzzStrategyBuilder<Program, Args, Wit>
where
    Program: FuzzableProgram<Program> + ProgramFactory<Program> + Clone + 'static,
    Args: ArgumentsTrait + RandomArguments + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
{
    pub fn with_signer(mut self, signer: Signer) -> Self {
        self.signer_option = SignerOption::CustomSigner;
        self.signer = Some(signer);
        self
    }

    pub fn with_default_signer(mut self) -> Self {
        self.signer_option = SignerOption::DefaultTestConfigSigner;
        self
    }

    pub fn with_no_signer(mut self) -> Self {
        self.signer_option = SignerOption::NoSigning;
        self
    }
}

impl<Program, Args, Wit> FuzzStrategyBuilder<Program, Args, Wit>
where
    Program: FuzzableProgram<Program> + ProgramFactory<Program> + Clone + 'static,
    Args: ArgumentsTrait + RandomArguments + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
{
    pub fn build(
        self,
        strategy_storage: impl Strategy<Value = (Arguments, WitnessValues)> + 'static,
        blueprint: impl ContractFuzzStrategyBlueprint<Program, Args, Wit> + 'static,
    ) -> SimplexFuzzEngine<Program, Args, Wit> {
        // TODO: add fetching of failure values in order to feed correct seed into testrunnner
        //  let mut runner_config = self.runner.config().clone();
        //         runner_config.cases = worker_runs;
        //         let mut runner = if let Some(seed) = self.config.seed {
        //             // For deterministic parallel fuzzing, derive a unique seed for each worker
        //             let worker_seed = if worker_id == 0 {
        //                 // Master worker uses the provided seed as is.
        //                 seed
        //             } else {
        //                 // Derive a worker-specific seed using keccak256(seed || worker_id)
        //                 let seed_data =
        //                     [&seed.to_be_bytes::<32>()[..], &worker_id.to_be_bytes()[..]].concat();
        //                 U256::from_be_bytes(keccak256(seed_data).0)
        //             };
        //             trace!(target: "forge::test", ?worker_seed, "deterministic seed for worker {worker_id}");
        //             let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &worker_seed.to_be_bytes::<32>());
        //             TestRunner::new_with_rng(runner_config, rng)
        //         } else {
        //             TestRunner::new(runner_config)
        //         };

        SimplexFuzzEngine {
            runner: proptest::test_runner::TestRunner::new(self.proptest_config),
            fuzz_context: FuzzContext {
                #[allow(clippy::arc_with_non_send_sync)]
                signer: Arc::new(self.signer),
                mock_provider: self.mock_provider,
                #[allow(clippy::arc_with_non_send_sync)]
                test_context: Arc::new(self.test_context),
                signer_option: self.signer_option,
                network: self.network,
            },
            local_fuzz_config: LocalFuzzConfig { runs: 500 },
            strategy_storage: strategy_storage.boxed(),
            blueprint: Box::new(blueprint),
        }
    }
}

pub struct StrategyStorageBuilder<Args, Wit, BaseStrat> {
    base_strat: Option<BaseStrat>,
    _placeholder: PhantomData<(Args, Wit)>,
}

impl<Args, Wit> StrategyStorageBuilder<Args, Wit, ()> {
    pub fn new() -> Self {
        Self {
            base_strat: None,
            _placeholder: Default::default(),
        }
    }
}

impl<Args, Wit, BaseStrat> StrategyStorageBuilder<Args, Wit, BaseStrat> {
    pub fn with_random(self) -> StrategyStorageBuilder<Args, Wit, Random<Args, Wit>> {
        StrategyStorageBuilder {
            base_strat: Some(Random::<Args, Wit>::default()),
            _placeholder: Default::default(),
        }
    }

    pub fn with_random_pool(self) -> StrategyStorageBuilder<Args, Wit, RandomValuePool<Args, Wit>> {
        StrategyStorageBuilder {
            base_strat: Some(RandomValuePool::<Args, Wit>::default()),
            _placeholder: Default::default(),
        }
    }

    pub fn with_custom_strategy<NewStrat>(self, custom_strat: NewStrat) -> StrategyStorageBuilder<Args, Wit, NewStrat>
    where
        NewStrat: Strategy<Value = (Arguments, WitnessValues)> + 'static,
    {
        StrategyStorageBuilder {
            base_strat: Some(custom_strat),
            _placeholder: Default::default(),
        }
    }

    pub fn with_random_asset_value(self) -> StrategyStorageBuilder<Args, Wit, BaseStrat> {
        StrategyStorageBuilder {
            base_strat: self.base_strat,
            _placeholder: Default::default(),
        }
    }
}

impl<Args, Wit, BaseStrat> StrategyStorageBuilder<Args, Wit, BaseStrat>
where
    BaseStrat: Strategy<Value = (Arguments, WitnessValues)> + 'static,
{
    pub fn build(self) -> BoxedStrategy<((Arguments, WitnessValues))> {
        let base = self
            .base_strat
            .expect("Base strategy is mandatory. Call with_random() or similar.");
        base.boxed()
    }
}

// TODO: create a shared state with Atomic counter

#[derive(Debug)]
pub struct CaseOutcome {}

/// Returned by a single fuzz when a counterexample has been discovered
#[derive(Debug)]
pub struct CounterExampleOutcome {
    pub args: Arguments,
    pub wit: WitnessValues,
}

/// Outcome of a single fuzz
#[derive(Debug)]
#[expect(clippy::large_enum_variant)]
pub enum FuzzOutcome {
    Case(CaseOutcome),
    CounterExample(CounterExampleOutcome),
}

impl<Program, Args, Wit> SimplexFuzzEngine<Program, Args, Wit>
where
    Program: FuzzableProgram<Program> + ProgramFactory<Program> + Clone + 'static,
{
    pub fn run_with_check(self, program_post_hook: impl ProgramCheck) {
        let mut runner = self.runner;
        let context = self.fuzz_context;
        let blueprint = self.blueprint;
        let strategy_storage = self.strategy_storage;

        let fuzz_tx_blueprint = blueprint.get_initial_ft();
        let program_check = Box::new(program_post_hook);

        let mut runner_cnt = 0;
        'stop: while runner_cnt < self.local_fuzz_config.runs {
            // TODO: If counterexample recorded, replay it first, without incrementing runs.
            //  single_failed_test_case()
            // TODO: evaluate incremental runs
            let mut inc_runs = || {
                debug_assert!(
                    runner_cnt <= self.local_fuzz_config.runs,
                    "worker runs were not distributed correctly"
                );
                runner_cnt += 1;
            };

            // v1
            let init_strategy =
                BlueprintFuzzStrategy::<Program>::new(strategy_storage.clone(), fuzz_tx_blueprint.clone()).boxed();
            match Self::single_fuzz(&mut runner, &context, &program_check, &init_strategy) {
                // v2
                // match Self::single_fuzz_2(
                //     &mut runner,
                //     &context,
                //     fuzz_tx_blueprint.clone(),
                //     &program_check,
                //     &strategy_storage,
                // ) {
                Ok(fuzz_outcome) => match fuzz_outcome {
                    FuzzOutcome::Case(case) => {
                        inc_runs();
                    }
                    FuzzOutcome::CounterExample(CounterExampleOutcome { args, wit }) => {
                        panic!("program failed tith these arguments: {args} and witness: {wit}");
                        break 'stop;
                    }
                },
                Err(err) => match err {
                    TestCaseError::Fail(e) => {
                        panic!("{e}");
                    }
                    TestCaseError::Reject(e) => {
                        panic!("{e}");
                    }
                },
            }
        }
    }

    fn single_fuzz(
        test_runner: &mut TestRunner,
        fuzz_context: &FuzzContext,
        program_post_hook: &Box<impl ProgramCheck>,
        init_ft_strategy: &BoxedStrategy<(Arguments, WitnessValues, FinalTransaction)>,
    ) -> Result<FuzzOutcome, TestCaseError> {
        // TODO: extract seed in order to save it in failure case

        // TODO: extract insertion of witness into strategies
        let (arguments, witness, final_transaction) = init_ft_strategy
            .new_tree(test_runner)
            .map_err(|e| TestCaseError::fail(format!("Failed to generate new tree: {:?}", e)))?
            .current();
        let pst = fuzz_context.sign_or_extract(&final_transaction)?;

        // Check program
        let (failure_program, _script) = Program::build_program(arguments.clone(), &fuzz_context.network);
        // Iterate over program inputs to check only appropriate items
        for (i, input) in final_transaction.inputs().iter().enumerate() {
            if input.program_input.as_ref().is_some() {
                let exec_result: ProgramExecResult =
                    failure_program
                        .as_ref()
                        .as_ref()
                        .execute(&pst, &witness, i, &fuzz_context.network);
                if let Err(e) = program_post_hook.call(fuzz_context, &pst, &arguments, &witness, i, exec_result) {
                    return Err(TestCaseError::fail(format!("{e}, args: {arguments}, wit: {witness}")));
                }
            }
        }
        Ok(FuzzOutcome::Case(CaseOutcome {}))
    }

    fn single_fuzz_2(
        test_runner: &mut TestRunner,
        fuzz_context: &FuzzContext,
        initial_tx: BlueprintDraftConstructor,
        program_post_hook: &Box<impl ProgramCheck>,
        strategy: &BoxedStrategy<(Arguments, WitnessValues)>,
    ) -> Result<FuzzOutcome, TestCaseError> {
        // TODO: extract seed in order to save it in failure case
        let (arguments, witness, final_transaction) =
            initial_tx.insert_real_program_values::<Program>(test_runner, strategy);
        let pst = fuzz_context.sign_or_extract(&final_transaction)?;

        let (failure_program, _script) = Program::build_program(arguments.clone(), &fuzz_context.network);

        // Iterate over program inputs to sign only appropriate items
        for (i, input) in final_transaction.inputs().iter().enumerate() {
            if input.program_input.as_ref().is_some() {
                let exec_result: ProgramExecResult =
                    failure_program
                        .as_ref()
                        .as_ref()
                        .execute(&pst, &witness, i, &fuzz_context.network);
                if let Err(e) = program_post_hook.call(fuzz_context, &pst, &arguments, &witness, i, exec_result) {
                    return Err(TestCaseError::fail(format!("{e}, args: {arguments}, wit: {witness}")));
                }
            }
        }
        Ok(FuzzOutcome::Case(CaseOutcome {}))
    }
}
