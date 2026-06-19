use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::Arc;

use proptest::prelude::{BoxedStrategy, TestCaseError};
use proptest::strategy::Strategy;

use crate::context::TestContext;
use crate::mutantesting::core::{
    ContractFuzzStrategy, FuzzContext, FuzzableProgram, ProgramCheck, ProgramExecResult, SignerOption,
};
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::{Arguments, WitnessValues};
use smplx_sdk::program::{ArgumentsTrait, ProgramTrait, RandomArguments, RandomWitness, WitnessTrait};
use smplx_sdk::provider::{EsploraProvider, SimplicityNetwork};
use smplx_sdk::signer::{Signer, SignerError};
use smplx_sdk::transaction::FinalTransaction;

pub struct SimplexFuzzEngineInner<Program> {
    pub(crate) fuzz_context: FuzzContext,
    pub(crate) strategy_storage: Option<BoxedStrategy<(Arguments, WitnessValues, FinalTransaction)>>,
    _phantom: PhantomData<Program>,
}

pub struct SimplexFuzzEngine<Program> {
    runner: RefCell<proptest::test_runner::TestRunner>,
    inner: RefCell<SimplexFuzzEngineInner<Program>>,
}

#[allow(clippy::arc_with_non_send_sync)]
impl Default for FuzzContext {
    fn default() -> Self {
        let default_network = SimplicityNetwork::default_regtest();
        Self {
            signer: Arc::new(None),
            network: default_network,
            mock_provider: Arc::new(get_default_provider(default_network)),
            test_context: Arc::new(None),
            signer_option: SignerOption::NoSigning,
        }
    }
}

impl FuzzContext {
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn with_signer(&mut self, signer: Signer) {
        self.signer_option = SignerOption::CustomSigner;
        self.signer = Arc::new(Some(signer));
    }

    #[allow(clippy::arc_with_non_send_sync)]
    pub fn with_default_signer(&mut self) {
        self.signer_option = SignerOption::DefaultTestConfigSigner;
    }

    #[inline]
    pub fn sign_or_extract(&self, ft: &FinalTransaction) -> Result<PartiallySignedTransaction, SignerError> {
        match self.signer_option {
            SignerOption::DefaultTestConfigSigner | SignerOption::CustomSigner => {
                let signer = self.get_signer();
                Ok(signer.unwrap().sign_tx_raw(ft)?)
            }
            SignerOption::NoSigning => Ok(ft.extract_pst().0),
        }
    }
}
fn get_default_provider(default_network: SimplicityNetwork) -> EsploraProvider {
    EsploraProvider::new("default_web_page.com".into(), default_network)
}

impl<Program> SimplexFuzzEngine<Program>
where
    Program: FuzzableProgram<Program> + Clone + 'static,
{
    pub fn from_config(mut config: proptest::test_runner::Config) -> Self {
        config.cases = 500;
        Self {
            runner: RefCell::new(proptest::test_runner::TestRunner::new(config)),
            inner: RefCell::new(SimplexFuzzEngineInner {
                fuzz_context: FuzzContext::default(),
                strategy_storage: None,
                _phantom: PhantomData,
            }),
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
            runner: RefCell::new(proptest::test_runner::TestRunner::new(config)),
            inner: RefCell::new(SimplexFuzzEngineInner {
                fuzz_context: FuzzContext {
                    #[allow(clippy::arc_with_non_send_sync)]
                    signer: Arc::new(None),
                    #[allow(clippy::arc_with_non_send_sync)]
                    mock_provider: Arc::new(get_default_provider(default_network)),
                    #[allow(clippy::arc_with_non_send_sync)]
                    test_context: Arc::new(Some(test_context)),
                    signer_option: SignerOption::NoSigning,
                    network: SimplicityNetwork::Liquid,
                },
                strategy_storage: None,
                _phantom: PhantomData,
            }),
        }
    }

    pub fn with_signer(&self, signer: Signer) {
        self.inner.borrow_mut().fuzz_context.with_signer(signer);
    }

    pub fn with_default_signer(&self) {
        self.inner.borrow_mut().fuzz_context.with_default_signer();
    }

    pub fn with_final_arg_gen_strategy<Args, Wit, S>(
        &self,
        arg_gen: impl Strategy<Value = (Arguments, WitnessValues)> + 'static,
        ft_gen: S,
    ) where
        Args: ArgumentsTrait + RandomArguments + std::fmt::Debug + Clone + 'static,
        Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
        S: ContractFuzzStrategy<Program, Args, Wit, AdditionalInput = ()> + 'static,
    {
        let context = self.inner.borrow().fuzz_context.clone();
        let mapped = arg_gen.prop_map(move |(args, wit)| ft_gen.gen_final_transaction(context.clone(), args, wit, ()));
        self.inner.borrow_mut().strategy_storage = Some(mapped.boxed());
    }

    pub fn with_final_arg_gen_strategy_ext<Args, Wit, S>(
        &self,
        arg_gen: impl Strategy<Value = ((Arguments, WitnessValues), S::AdditionalInput)> + 'static,
        ft_gen: S,
    ) where
        Args: ArgumentsTrait + RandomArguments + std::fmt::Debug + Clone + 'static,
        Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
        S: ContractFuzzStrategy<Program, Args, Wit> + 'static,
    {
        let context = self.inner.borrow().fuzz_context.clone();
        let mapped = arg_gen.prop_map(move |((args, wit), additional)| {
            ft_gen.gen_final_transaction(context.clone(), args, wit, additional)
        });
        self.inner.borrow_mut().strategy_storage = Some(mapped.boxed());
    }

    pub fn run_with_check(self, program_check_fn: impl ProgramCheck) {
        let mut runner = self.runner.into_inner();
        let inner = self.inner.into_inner();

        let context = inner.fuzz_context;
        let strategy = inner.strategy_storage.expect("Strategy must be configured");

        match runner.run(&strategy, |(args, wit, ft)| {
            let pst = context.sign_or_extract(&ft).unwrap();

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
