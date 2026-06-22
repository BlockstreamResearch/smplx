use crate::mutantesting::core::{
    ArgGenFuzzStrategy, ArgGenFuzzStrategy2, FuzzContext, FuzzableBaseContextGen, FuzzableContextGen, FuzzableProgram,
    ProgramCheck, ProgramExecResult,
};
use crate::mutantesting::provider::MockProvider;
use proptest::prelude::{Strategy, TestCaseError};
use simplicityhl::{Arguments, WitnessValues};
use smplx_sdk::program::{ArgumentsTrait, ProgramTrait, WitnessTrait};
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::Arc;
use simplicityhl::elements::pset::PartiallySignedTransaction;

pub struct SimplexFuzzEngineInner<Program, Args, Wit> {
    pub(crate) fuzz_context: FuzzContext,
    pub(crate) strategy_storage: Option<Box<dyn ArgGenFuzzStrategy<Args, Wit>>>,
    pub(crate) strategy_storage2: Option<Box<dyn ArgGenFuzzStrategy2<Args, Wit>>>,
    pub(crate) base_gen: Option<Box<dyn FuzzableBaseContextGen<Program>>>,
    pub(crate) mod_gen: Option<Box<dyn FuzzableContextGen<Program>>>,
}

pub struct SimplexFuzzEngine<Program, Args, Wit> {
    runner: RefCell<proptest::test_runner::TestRunner>,
    inner: RefCell<SimplexFuzzEngineInner<Program, Args, Wit>>,
    _phantom: PhantomData<Program>,
}

impl Default for FuzzContext {
    fn default() -> Self {
        let default_network = SimplicityNetwork::default_regtest();
        Self {
            signer: None,
            network: default_network,
            mock_provider: Arc::new(MockProvider {
                network: default_network,
            }),
        }
    }
}

impl FuzzContext {
    fn with_signer(&mut self, signer: Signer) {
        self.signer = Some(Arc::new(signer));
    }
}

impl<FuzzProgram, Args, Wit> SimplexFuzzEngine<FuzzProgram, Args, Wit>
where
    Args: ArgumentsTrait + Into<Arguments> + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + std::fmt::Debug + Clone + 'static,
    FuzzProgram: FuzzableProgram<FuzzProgram> + Clone + 'static,
{
    pub fn from_config(mut config: proptest::test_runner::Config, _phantom: PhantomData<FuzzProgram>) -> Self {
        // config.cases = 500;
        Self {
            runner: RefCell::new(proptest::test_runner::TestRunner::new(config)),
            inner: RefCell::new(SimplexFuzzEngineInner {
                fuzz_context: FuzzContext::default(),
                strategy_storage: None,
                strategy_storage2: None,
                base_gen: None,
                mod_gen: None,
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

    pub fn with_pset_base_gen_strategy<G>(&self)
    where
        G: FuzzableBaseContextGen<FuzzProgram> + Default + 'static,
    {
        self.inner.borrow_mut().base_gen = Some(Box::new(G::default()));
    }

    pub fn with_pset_strategy<G>(&self)
    where
        G: FuzzableContextGen<FuzzProgram> + Default + 'static,
    {
        self.inner.borrow_mut().mod_gen = Some(Box::new(G::default()));
    }

    pub fn with_arg_gen_strategy<S>(&self)
    where
        S: ArgGenFuzzStrategy<Args, Wit> + Default + 'static,
    {
        self.inner.borrow_mut().strategy_storage = Some(Box::new(S::default()));
    }

    pub fn with_arg_gen_strategy_plus_pst<S>(&self)
    where
        S: ArgGenFuzzStrategy2<Args, Wit> + Default + 'static,
    {
        self.inner.borrow_mut().strategy_storage2 = Some(Box::new(S::default()));
    }

    pub fn run_with_check(&self, program_check_fn: impl ProgramCheck) {
        let mut runner = self.runner.borrow_mut();
        let inner = self.inner.borrow();

        let base_gen = inner.base_gen.as_ref().expect("Base gen strategy must be configured");
        let modifier = inner.mod_gen.as_ref().expect("Mod gen strategy must be configured");
        let strategy_gen = inner.strategy_storage.as_ref().expect("Strategy must be configured");

        // TODO: remove strategies Vec, by now impossible to combine them, we can only use 1
        let strategy = strategy_gen.get_strategy(Arc::new(inner.fuzz_context.clone()));
        match runner.run(&strategy, |(args, wit)| {
            let context = &inner.fuzz_context;
            let ft = base_gen.build_base_transaction(&context.network, args.clone(), wit.clone());
            // TODO: maybe make a couple of modification for one ft if non-default used?
            let pst = modifier
                .modify_transaction(&inner.fuzz_context.signer, ft, &args, &wit)
                .unwrap();

            let (failure_program, _script) = FuzzProgram::build_program(args.clone(), &context.network);

            // TODO: think how to provide a pset for a program execution environment
            // let pst = PartiallySignedTransaction::from_tx(tx.clone());

            let exec_result: ProgramExecResult = failure_program.get_program().execute(&pst, &wit, 0, &context.network);

            match program_check_fn.call(context, &pst, &args, &wit, exec_result) {
                Ok(_) => Ok(()),
                Err(e) => Err(TestCaseError::fail(e)),
            }
        }) {
            Ok(()) => (),
            Err(e) => ::core::panic!("{}\n{}", e, runner),
        };
    }

    pub fn run_with_check_2(&self, program_check_fn: impl ProgramCheck) {
        let mut runner = self.runner.borrow_mut();
        let inner = self.inner.borrow();

        let base_gen = inner.base_gen.as_ref().expect("Base gen strategy must be configured");
        let strategy_gen = inner.strategy_storage.as_ref().expect("Strategy must be configured");

        let strategy = strategy_gen.get_strategy(Arc::new(inner.fuzz_context.clone()));
        let mut runnner_rng = runner.new_rng();
        match runner.run(&strategy, |(args, wit)| {
            let mut runnner_rng_local = runnner_rng.clone();
            let context = &inner.fuzz_context;
            let (ft, pst, args, wit) =
                base_gen.build_base_transaction_2(&context, args.clone(), wit.clone(), &mut runnner_rng_local);
            println!("args: {args}, wit: {wit}");
            let (failure_program, _script) = FuzzProgram::build_program(args.clone(), &context.network);

            let exec_result: ProgramExecResult = failure_program.get_program().execute(&pst, &wit, 0, &context.network);

            match program_check_fn.call(context, &pst, &args, &wit, exec_result) {
                Ok(_) => Ok(()),
                Err(e) => Err(TestCaseError::fail(e)),
            }
        }) {
            Ok(()) => (),
            Err(e) => ::core::panic!("{}\n{}", e, runner),
        };
    }

    pub fn run_with_check_3(self, program_check_fn: impl ProgramCheck) {
        //TODO: remove refcell
        let mut runner = self.runner.borrow_mut();
        let inner = self.inner.borrow();

        let strategy_gen = inner.strategy_storage2.as_ref().expect("Strategy2 must be configured");

        let strategy = strategy_gen.get_strategy(Arc::new(inner.fuzz_context.clone()));
        match runner.run(&strategy, |(args, wit, pst)| {
            let context = &inner.fuzz_context;
            println!("args: {args}, wit: {wit}");
            // TODO: how to determine correct input indexes??
            //  what if we have multiple inputs?, how to behave then?? another trait function?

            let (failure_program, _script) = FuzzProgram::build_program(args.clone(), &context.network);
            let exec_result: ProgramExecResult = failure_program.get_program().execute(&pst, &wit, 0, &context.network);

            match program_check_fn.call(context, &pst, &args, &wit, exec_result) {
                Ok(_) => Ok(()),
                Err(e) => Err(TestCaseError::fail(e)),
            }
        }) {
            Ok(()) => (),
            Err(e) => ::core::panic!("{}\n{}", e, runner),
        };
    }

    pub fn run_with_check_4(self, program_check_fn: impl ProgramCheck ) {
        //TODO: remove refcell
        let mut runner = self.runner.borrow_mut();
        let inner = self.inner.borrow();

        let strategy_gen = inner.strategy_storage2.as_ref().expect("Strategy2 must be configured");

        let strategy = strategy_gen.get_strategy(Arc::new(inner.fuzz_context.clone()));
        match runner.run(&strategy, |(args, wit, pst)| {
            let context = &inner.fuzz_context;
            println!("args: {args}, wit: {wit}");
            // TODO: how to determine correct input indexes??
            //  what if we have multiple inputs?, how to behave then?? another trait function?

            let (failure_program, _script) = FuzzProgram::build_program(args.clone(), &context.network);
            let exec_result: ProgramExecResult = failure_program.get_program().execute(&pst, &wit, 0, &context.network);

            match program_check_fn.call(context, &pst, &args, &wit, exec_result) {
                Ok(_) => Ok(()),
                Err(e) => Err(TestCaseError::fail(e)),
            }
        }) {
            Ok(()) => (),
            Err(e) => ::core::panic!("{}\n{}", e, runner),
        };
    }
}
