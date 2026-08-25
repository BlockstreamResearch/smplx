use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;

use simplicityhl::{Arguments, WitnessValues};

use proptest::prelude::{BoxedStrategy, TestCaseError};
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::TestRunner;

use smplx_sdk::program::{ArgumentsTrait, ProgramFactory, ProgramTrait, RandomArguments, RandomWitness, WitnessTrait};
use smplx_sdk::provider::{EsploraProvider, ProviderTrait, SimplicityNetwork};
use smplx_sdk::signer::Signer;

use crate::context::TestContext;
use crate::fuzz::args_strategy::{InterestingRandom, Random, RandomValuePool};
use crate::fuzz::builders::{FinalTransactionBuilder, ProgramTarget};
use crate::fuzz::core::{FuzzContext, FuzzableProgram, SignerOption};
use crate::fuzz::{ProgramCheck, ProgramExecResult};

pub struct SimplexFuzzEngine<Program, Args, Wit> {
    runner: TestRunner,
    fuzz_context: FuzzContext,
    local_fuzz_config: LocalFuzzConfig,
    strategy_storage: BoxedStrategy<(Arguments, WitnessValues)>,
    blueprint: FinalTransactionBuilder,
    _phantom: PhantomData<(Program, Args, Wit)>,
}

pub struct LocalFuzzConfig {
    runs: usize,
}

pub struct FuzzEngineBuilder<Program, Args, Wit> {
    proptest_config: proptest::test_runner::Config,
    pub(crate) signer_option: SignerOption,
    pub(crate) signer: Option<Signer>,
    test_context: Option<TestContext>,
    mock_provider: Arc<dyn ProviderTrait>,
    network: SimplicityNetwork,
    _phantom: PhantomData<(Program, Args, Wit)>,
}

impl<Program, Args, Wit> FuzzEngineBuilder<Program, Args, Wit>
where
    Program: FuzzableProgram<Program> + ProgramFactory<Program> + Clone + 'static,
    Args: ArgumentsTrait + RandomArguments + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
{
    pub fn new(config: proptest::test_runner::Config) -> Self {
        let default_network = SimplicityNetwork::default_regtest();

        Self {
            proptest_config: config,
            test_context: None,
            signer_option: SignerOption::NoSigning,
            signer: None,
            mock_provider: Arc::new(Self::get_default_provider(default_network)),
            network: default_network,
            _phantom: PhantomData,
        }
    }

    pub fn from_context(mut config: proptest::test_runner::Config, test_context: TestContext) -> Self {
        let default_network = SimplicityNetwork::default_regtest();
        let smplx_test_context = test_context.get_config();
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
            test_context: Some(test_context),
            signer_option: SignerOption::NoSigning,
            signer: None,
            mock_provider: Arc::new(Self::get_default_provider(default_network)),
            network: default_network,
            _phantom: PhantomData,
        }
    }

    fn get_default_provider(default_network: SimplicityNetwork) -> EsploraProvider {
        EsploraProvider::new("default_web_page.com".into(), default_network)
    }
}

impl<Program, Args, Wit> FuzzEngineBuilder<Program, Args, Wit>
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

    pub fn build(
        self,
        strategy_storage: impl Strategy<Value = (Arguments, WitnessValues)> + 'static,
        blueprint: FinalTransactionBuilder,
    ) -> SimplexFuzzEngine<Program, Args, Wit> {
        // TODO: add fetching of failure values to feed correct seed into TestRunner on creation

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
            blueprint,
            _phantom: PhantomData,
        }
    }
}

pub struct FuzzStrategyBuilder<Args, Wit, BaseStrat> {
    base_strat: Option<BaseStrat>,
    _placeholder: PhantomData<(Args, Wit)>,
}

impl<Args, Wit> FuzzStrategyBuilder<Args, Wit, ()> {
    pub fn new() -> Self {
        Self {
            base_strat: None,
            _placeholder: Default::default(),
        }
    }
}

impl<Args, Wit> Default for FuzzStrategyBuilder<Args, Wit, ()> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Args, Wit, BaseStrat> FuzzStrategyBuilder<Args, Wit, BaseStrat> {
    pub fn with_random(self) -> FuzzStrategyBuilder<Args, Wit, Random<Args, Wit>> {
        FuzzStrategyBuilder {
            base_strat: Some(Random::<Args, Wit>::default()),
            _placeholder: Default::default(),
        }
    }

    pub fn with_random_pool(self) -> FuzzStrategyBuilder<Args, Wit, RandomValuePool<Args, Wit>> {
        FuzzStrategyBuilder {
            base_strat: Some(RandomValuePool::<Args, Wit>::default()),
            _placeholder: Default::default(),
        }
    }

    pub fn with_custom_strategy<NewStrat>(self, custom_strat: NewStrat) -> FuzzStrategyBuilder<Args, Wit, NewStrat>
    where
        NewStrat: Strategy<Value = (Arguments, WitnessValues)> + 'static,
    {
        FuzzStrategyBuilder {
            base_strat: Some(custom_strat),
            _placeholder: Default::default(),
        }
    }

    pub fn with_random_interesting_values(self) -> FuzzStrategyBuilder<Args, Wit, InterestingRandom<Args, Wit>> {
        FuzzStrategyBuilder {
            base_strat: Some(InterestingRandom::<Args, Wit>::default()),
            _placeholder: Default::default(),
        }
    }
}

impl<Args, Wit, BaseStrat> FuzzStrategyBuilder<Args, Wit, BaseStrat>
where
    BaseStrat: Strategy<Value = (Arguments, WitnessValues)> + 'static,
{
    pub fn build(self) -> BoxedStrategy<(Arguments, WitnessValues)> {
        let base = self
            .base_strat
            .expect("Base strategy is mandatory. Call with_random() or similar.");
        base.boxed()
    }
}

// TODO: create a shared state with Atomic counter (for multiple runners)

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

        // TODO: add collected strategies usage to modify FinalTransaction or inputs into `proptest::prop_oneof[ strategy list ... ]`

        let mut runner_cnt = 0;
        while runner_cnt < self.local_fuzz_config.runs {
            // TODO: If counterexample recorded, replay it first, without incrementing runs.
            // TODO: evaluate incremental runs
            let mut inc_runs = || {
                debug_assert!(
                    runner_cnt <= self.local_fuzz_config.runs,
                    "worker runs were not distributed correctly"
                );
                runner_cnt += 1;
            };

            match Self::single_fuzz(&mut runner, &context, &blueprint, &program_post_hook, &strategy_storage) {
                Ok(fuzz_outcome) => match fuzz_outcome {
                    FuzzOutcome::Case(_case) => {
                        inc_runs();
                    }
                    FuzzOutcome::CounterExample(CounterExampleOutcome { args, wit }) => {
                        panic!("program failed tith these arguments: {args} and witness: {wit}");
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
        initial_tx: &FinalTransactionBuilder,
        program_post_hook: &impl ProgramCheck,
        strategy: &BoxedStrategy<(Arguments, WitnessValues)>,
    ) -> Result<FuzzOutcome, TestCaseError> {
        // TODO: extract seed in order to save it in failure case
        let (arguments, witness) = strategy
            .new_tree(test_runner)
            .map_err(|error| TestCaseError::fail(format!("failed to generate fuzz inputs: {error}")))?
            .current();
        let (program, script) = Program::build_program(arguments.clone(), &fuzz_context.network);
        let final_transaction = initial_tx
            .prepare_transaction(program.as_ref().as_ref(), &script, &arguments, &witness)
            .map_err(|error| TestCaseError::fail(format!("failed to prepare fuzz transaction: {error}")))?;
        let pst = fuzz_context.sign_or_extract(&final_transaction)?;

        // Iterate over program inputs to check contract execution
        for target in initial_tx.targets().iter().copied() {
            let ProgramTarget::Input(input_index) = target else {
                continue;
            };

            let exec_result: ProgramExecResult =
                program
                    .as_ref()
                    .as_ref()
                    .execute(&pst, &witness, input_index, &fuzz_context.network);
            if let Err(error) =
                program_post_hook.call(fuzz_context, &pst, &arguments, &witness, input_index, exec_result)
            {
                return Err(TestCaseError::fail(format!(
                    "{error}, args: {arguments}, wit: {witness}"
                )));
            }
        }
        Ok(FuzzOutcome::Case(CaseOutcome {}))
    }
}
