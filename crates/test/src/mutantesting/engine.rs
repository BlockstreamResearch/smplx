use crate::mutantesting::core::{FuzzContext, FuzzStrategy, FuzzableProgram, ProgramCheck, ProgramExecResult};
use crate::mutantesting::provider::MockProvider;
use proptest::prelude::TestCaseError;
use simplicityhl::Arguments;
use smplx_sdk::program::{ArgumentsTrait, ProgramTrait, WitnessTrait};
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::Arc;

pub struct SimplexFuzzEngineInner<Program, Args, Wit> {
    pub(crate) fuzz_context: FuzzContext,
    pub(crate) strategy_storage: Option<Box<dyn FuzzStrategy<Program, Args, Wit>>>,
}

pub struct SimplexFuzzEngine<Program, Args, Wit> {
    runner: RefCell<proptest::test_runner::TestRunner>,
    inner: RefCell<SimplexFuzzEngineInner<Program, Args, Wit>>,
    _phantom: PhantomData<Program>,
}

#[allow(clippy::arc_with_non_send_sync)]
impl Default for FuzzContext {
    fn default() -> Self {
        let default_network = SimplicityNetwork::default_regtest();
        Self {
            signer: Arc::new(None),
            network: default_network,
            mock_provider: Arc::new(MockProvider {
                network: default_network,
            }),
        }
    }
}

impl FuzzContext {
    #[allow(clippy::arc_with_non_send_sync)]
    fn with_signer(&mut self, signer: Signer) {
        self.signer = Arc::new(Some(signer));
    }
}

impl<Program, Args, Wit> SimplexFuzzEngine<Program, Args, Wit>
where
    Args: ArgumentsTrait + Into<Arguments> + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + std::fmt::Debug + Clone + 'static,
    Program: FuzzableProgram<Program> + Clone + 'static,
{
    pub fn from_config(mut config: proptest::test_runner::Config, _phantom: PhantomData<Program>) -> Self {
        config.cases = 500;
        Self {
            runner: RefCell::new(proptest::test_runner::TestRunner::new(config)),
            inner: RefCell::new(SimplexFuzzEngineInner {
                fuzz_context: FuzzContext::default(),
                strategy_storage: None,
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
        self.inner
            .borrow_mut()
            .fuzz_context
            .with_signer(Signer::new(DEFAULT_TEST_MNEMONIC, Box::new(MockProvider { network })));
    }

    pub fn with_arg_gen_strategy<S>(&self)
    where
        S: FuzzStrategy<Program, Args, Wit> + Default + 'static,
    {
        self.inner.borrow_mut().strategy_storage = Some(Box::new(S::default()));
    }

    pub fn run_with_check(self, program_check_fn: impl ProgramCheck) {
        let mut runner = self.runner.borrow_mut();
        let inner = self.inner.borrow();

        let strategy_gen = inner.strategy_storage.as_ref().expect("Strategy must be configured");
        let context = inner.fuzz_context.clone();
        let strategy = strategy_gen.get_strategy(context.clone());

        match runner.run(&strategy, |(args, wit, pst)| {
            let (failure_program, _script) = Program::build_program(args.clone(), &context.network);

            let exec_result: ProgramExecResult =
                failure_program
                    .as_ref()
                    .as_ref()
                    .execute(&pst, &wit, 0, &context.network);

            match program_check_fn.call(&context, &pst, &args, &wit, exec_result) {
                Ok(_) => Ok(()),
                Err(e) => Err(TestCaseError::fail(e)),
            }
        }) {
            Ok(()) => (),
            Err(e) => ::core::panic!("{}\n{}", e, runner),
        };
    }
}
