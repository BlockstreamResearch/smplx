use crate::mutantesting::core::{
    FuzzContext, FuzzStrategy, FuzzableProgram, GenStrategy, GenStrategyExt, ProgramCheck, ProgramExecResult,
    UserFuzzStrategy, UserFuzzStrategyExt,
};
use crate::mutantesting::sign_or_extract;
use proptest::prelude::TestCaseError;
use proptest::strategy::Strategy;
use simplicityhl::{Arguments, WitnessValues};
use smplx_sdk::program::{ArgumentsTrait, ProgramTrait, RandomArguments, RandomWitness, WitnessTrait};
use smplx_sdk::provider::{EsploraProvider, SimplicityNetwork};
use smplx_sdk::signer::Signer;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::Arc;

pub struct SimplexFuzzEngineInner<Program, Args, Wit, T = ()> {
    pub(crate) fuzz_context: FuzzContext,
    pub(crate) strategy_storage: Option<GenStrategy<Program, Args, Wit>>,
    pub(crate) strategy_storage_ext: Option<GenStrategyExt<Program, Args, Wit, T>>,
}

pub struct SimplexFuzzEngine<Program, Args, Wit, T = ()> {
    runner: RefCell<proptest::test_runner::TestRunner>,
    inner: RefCell<SimplexFuzzEngineInner<Program, Args, Wit, T>>,
    _phantom: PhantomData<Program>,
}

#[allow(clippy::arc_with_non_send_sync)]
impl Default for FuzzContext {
    fn default() -> Self {
        let default_network = SimplicityNetwork::default_regtest();
        Self {
            signer: Arc::new(None),
            network: default_network,
            mock_provider: Arc::new(get_default_provider(default_network)),
        }
    }
}

impl FuzzContext {
    #[allow(clippy::arc_with_non_send_sync)]
    fn with_signer(&mut self, signer: Signer) {
        self.signer = Arc::new(Some(signer));
    }
}
fn get_default_provider(default_network: SimplicityNetwork) -> EsploraProvider {
    EsploraProvider::new("default_web_page.com".into(), default_network)
}

impl<Program, Args, Wit, T> SimplexFuzzEngine<Program, Args, Wit, T>
where
    Args: ArgumentsTrait + RandomArguments + Into<Arguments> + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
    Program: FuzzableProgram<Program> + Clone + 'static,
    T: std::fmt::Debug + 'static,
{
    pub fn from_config(mut config: proptest::test_runner::Config, _phantom: PhantomData<Program>) -> Self {
        config.cases = 500;
        Self {
            runner: RefCell::new(proptest::test_runner::TestRunner::new(config)),
            inner: RefCell::new(SimplexFuzzEngineInner {
                fuzz_context: FuzzContext::default(),
                strategy_storage: None,
                strategy_storage_ext: None,
            }),
            _phantom,
        }
    }

    pub fn with_signer(&self, signer: Signer) {
        self.inner.borrow_mut().fuzz_context.with_signer(signer);
    }

    pub fn with_default_signer(&self) {
        const DEFAULT_TEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";

        let network = self.inner.borrow().fuzz_context.network;
        self.inner.borrow_mut().fuzz_context.with_signer(Signer::new(
            DEFAULT_TEST_MNEMONIC,
            Box::new(get_default_provider(network)),
        ));
    }

    pub fn with_final_arg_gen_strategy(
        &self,
        arg_gen: impl Strategy<Value = (Arguments, WitnessValues)> + 'static,
        ft_gen: impl UserFuzzStrategy<Program, Args, Wit> + 'static,
    ) {
        self.inner.borrow_mut().strategy_storage = Some((arg_gen.boxed(), Box::new(ft_gen)));
    }

    pub fn with_final_arg_gen_strategy_ext(
        &self,
        arg_gen: impl Strategy<Value = ((Arguments, WitnessValues), T)> + 'static,
        ft_gen: impl UserFuzzStrategyExt<Program, Args, Wit, T> + 'static,
    ) {
        self.inner.borrow_mut().strategy_storage_ext = Some((arg_gen.boxed(), Box::new(ft_gen)));
    }

    pub fn run_with_check(self, program_check_fn: impl ProgramCheck) {
        let mut runner = self.runner.into_inner();
        let inner = self.inner.into_inner();

        let context = inner.fuzz_context;

        let strategy = if let Some(_) = inner.strategy_storage
            && let Some(_) = inner.strategy_storage_ext
        {
            panic!("Strategy must be only one");
        } else if let Some(strategy_gen) = inner.strategy_storage {
            strategy_gen.get_strategy(context.clone())
        } else if let Some(strategy_gen_ext) = inner.strategy_storage_ext {
            strategy_gen_ext.get_strategy(context.clone())
        } else {
            panic!("Strategy must be configured");
        };

        match runner.run(&strategy, |(args, wit, ft)| {
            let signer = context.signer.as_ref();
            let pst = sign_or_extract(signer, &ft).unwrap();

            let (failure_program, _script) = Program::build_program(args.clone(), &context.network);

            // Iterate over program inputs to sign only appropriate items
            for (i, input) in ft.inputs().iter().enumerate() {
                if input.program_input.as_ref().is_some() {
                    let exec_result: ProgramExecResult =
                        failure_program
                            .as_ref()
                            .as_ref()
                            .execute(&pst, &wit, i, &context.network);
                    if let Err(e) = program_check_fn.call(&context, &pst, &args, &wit, i, exec_result) {
                        return Err(TestCaseError::fail(e));
                    }
                }
            }
            Ok(())
        }) {
            Ok(()) => (),
            Err(e) => ::core::panic!("{}\n{}", e, runner),
        };
    }
}
