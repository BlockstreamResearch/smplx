use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;

use proptest::prelude::{BoxedStrategy, Just, TestCaseError};
use proptest::strategy::Strategy;

use crate::context::TestContext;
use crate::mutantesting::args_strategy::{Random, RandomValuePool};
use crate::mutantesting::core::{
    ContractFuzzStrategy, ContractFuzzStrategyBlueprint, FuzzContext, FuzzableProgram, SignerOption,
};
use crate::mutantesting::{ProgramCheck, ProgramExecResult};
use simplicityhl::elements::hashes::Hash;
use simplicityhl::{Arguments, WitnessValues};
use smplx_sdk::program::{ArgumentsTrait, ProgramTrait, RandomArguments, RandomWitness, WitnessTrait};
use smplx_sdk::provider::{EsploraProvider, ProviderTrait, SimplicityNetwork};
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::{FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature, UTXO};

pub struct SimplexFuzzEngine<Program, Args, Wit, AdditionalValue> {
    runner: proptest::test_runner::TestRunner,
    fuzz_context: FuzzContext,
    strategy_storage: BoxedStrategy<((Arguments, WitnessValues), AdditionalValue)>,
    blueprint: Box<dyn ContractFuzzStrategyBlueprint<Program, Args, Wit, AdditionalInput = AdditionalValue>>,
}

pub fn get_default_provider(default_network: SimplicityNetwork) -> EsploraProvider {
    EsploraProvider::new("default_web_page.com".into(), default_network)
}

pub struct FuzzStrategyBuilder<Program, Args, Wit, Add = ()> {
    proptest_config: proptest::test_runner::Config,
    pub(crate) strategy_storage: Option<BoxedStrategy<(Arguments, WitnessValues, Add)>>,
    pub(crate) blueprint: Option<Box<dyn ContractFuzzStrategyBlueprint<Program, Args, Wit, AdditionalInput = Add>>>,
    pub(crate) signer_option: SignerOption,
    pub(crate) signer: Option<Signer>,
    test_context: Option<TestContext>,
    mock_provider: Arc<dyn ProviderTrait>,
    network: SimplicityNetwork,
    _phantom: PhantomData<(Program, Args, Wit)>,
}

impl<Program, Args, Wit, Add> FuzzStrategyBuilder<Program, Args, Wit, Add>
where
    Program: FuzzableProgram<Program> + Clone + 'static,
    Args: ArgumentsTrait + RandomArguments + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
    Add: std::fmt::Debug + Clone + 'static,
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

impl<Program, Args, Wit, AdditionalValue> FuzzStrategyBuilder<Program, Args, Wit, AdditionalValue>
where
    Program: FuzzableProgram<Program> + Clone + 'static,
    Args: ArgumentsTrait + RandomArguments + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
    AdditionalValue: Debug + Clone,
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

impl<Program, Args, Wit, AdditionalValue> FuzzStrategyBuilder<Program, Args, Wit, AdditionalValue>
where
    Program: FuzzableProgram<Program> + Clone + 'static,
    Args: ArgumentsTrait + RandomArguments + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
    AdditionalValue: Debug + Clone,
{
    pub fn build(
        self,
        strategy_storage: impl Strategy<Value = ((Arguments, WitnessValues), AdditionalValue)> + 'static,
        blueprint: impl ContractFuzzStrategyBlueprint<Program, Args, Wit, AdditionalInput = AdditionalValue> + 'static,
    ) -> SimplexFuzzEngine<Program, Args, Wit, AdditionalValue> {
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
            strategy_storage: strategy_storage.boxed(),
            blueprint: Box::new(blueprint),
        }
    }
}

pub struct FuzzTxBlueprint<Program, Args, Wit, Add = ()> {
    pub(crate) steps:
        Vec<Arc<dyn Fn(&mut FinalTransaction, &FuzzContext, &Arguments, &WitnessValues, &Add) + Send + Sync + 'static>>,
    _phantom: PhantomData<(Program, Args, Wit)>,
}

impl<Program, Args, Wit, S> ContractFuzzStrategyBlueprint<Program, Args, Wit> for S
where
    S: ContractFuzzStrategy<Program, Args, Wit> + 'static,
    Program: FuzzableProgram<Program> + Clone + 'static,
    Args: ArgumentsTrait + RandomArguments + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
    S::AdditionalInput: std::fmt::Debug + Clone + 'static,
{
    type AdditionalInput = S::AdditionalInput;

    fn get_final_tx(
        &self,
        context: &FuzzContext,
        args: &Arguments,
        wit: &WitnessValues,
        additional: &Self::AdditionalInput,
    ) -> FinalTransaction {
        let (_, _, ft) = self.gen_final_transaction(context.clone(), args.clone(), wit.clone(), additional.clone());
        ft
    }
}

pub struct FnBlueprintWrapper<F, Args, Wit, Add> {
    f: F,
    _phantom: PhantomData<(Args, Wit, Add)>,
}

impl<Program, Args, Wit, F, AdditionalInput> ContractFuzzStrategyBlueprint<Program, Args, Wit>
    for FnBlueprintWrapper<F, Args, Wit, AdditionalInput>
where
    F: Fn(&FuzzContext, &Arguments, &WitnessValues, &AdditionalInput) -> FinalTransaction + 'static,
    Program: FuzzableProgram<Program> + Clone + 'static,
    Args: ArgumentsTrait + RandomArguments + std::fmt::Debug + Clone + 'static,
    Wit: WitnessTrait + RandomWitness + std::fmt::Debug + Clone + 'static,
    AdditionalInput: std::fmt::Debug + Clone + 'static,
{
    type AdditionalInput = AdditionalInput;

    fn get_final_tx(
        &self,
        context: &FuzzContext,
        args: &Arguments,
        wit: &WitnessValues,
        additional: &Self::AdditionalInput,
    ) -> FinalTransaction {
        let ft = (self.f)(context, args, wit, additional);
        ft
    }
}

impl<Program, Args, Wit, Add> ContractFuzzStrategyBlueprint<Program, Args, Wit>
    for FuzzTxBlueprint<Program, Args, Wit, Add>
where
    Program: FuzzableProgram<Program> + Clone + 'static,
    Add: std::fmt::Debug + Clone + 'static,
{
    type AdditionalInput = Add;

    fn get_final_tx(
        &self,
        context: &FuzzContext,
        args: &Arguments,
        wit: &WitnessValues,
        additional: &Self::AdditionalInput,
    ) -> FinalTransaction {
        let mut ft = FinalTransaction::new();
        for step in &self.steps {
            step(&mut ft, context, args, wit, additional);
        }
        ft
    }
}

impl<Program, Args, Wit, Add> FuzzTxBlueprint<Program, Args, Wit, Add>
where
    Program: FuzzableProgram<Program> + Clone + 'static,
    Add: std::fmt::Debug + Clone + 'static,
{
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            _phantom: PhantomData,
        }
    }

    pub fn add_program_input(mut self, amount: u64) -> Self {
        self.steps
            .push(Arc::new(move |ft, context, arguments, witness, _additional| {
                let (prog, script) = Program::build_program(arguments.clone(), &context.network);
                let mut txout = simplicityhl::elements::TxOut::new_fee(amount, context.network.policy_asset());
                txout.script_pubkey = script;

                let partial_input = PartialInput::new(UTXO {
                    outpoint: simplicityhl::elements::OutPoint::new(simplicityhl::elements::Txid::all_zeros(), 0),
                    txout,
                    secrets: None,
                });
                let program_input = ProgramInput::new(Box::new(prog.as_ref().as_ref().clone()), witness.clone());

                ft.add_program_input(partial_input, program_input, RequiredSignature::None);
            }));
        self
    }

    pub fn add_static_input(mut self, partial_input: PartialInput, required_sig: RequiredSignature) -> Self {
        self.steps
            .push(Arc::new(move |ft, _context, _arguments, _witness, _additional| {
                ft.add_input(partial_input.clone(), required_sig.clone());
            }));
        self
    }

    pub fn add_static_output(mut self, partial_output: PartialOutput) -> Self {
        self.steps
            .push(Arc::new(move |ft, _context, _arguments, _witness, _additional| {
                ft.add_output(partial_output.clone());
            }));
        self
    }

    pub fn add_custom_step<F>(mut self, step: F) -> Self
    where
        F: Fn(&mut FinalTransaction, &FuzzContext, &Arguments, &WitnessValues, &Add) + Send + Sync + 'static,
    {
        self.steps.push(Arc::new(step));
        self
    }
}

pub struct StrategyStorageBuilder<Args, Wit, BaseStrat, AddStrat> {
    base_strat: Option<BaseStrat>,
    add_strat: Option<AddStrat>,
    _placeholder: PhantomData<(Args, Wit)>,
}

impl<Args, Wit> StrategyStorageBuilder<Args, Wit, (), Just<()>> {
    pub fn new() -> Self {
        Self {
            base_strat: None,
            add_strat: Some(Just(())),
            _placeholder: Default::default(),
        }
    }
}

impl<Program, Args, Wit, Add> Default for FuzzTxBlueprint<Program, Args, Wit, Add>
where
    Program: FuzzableProgram<Program> + Clone + 'static,
    Add: std::fmt::Debug + Clone + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Args, Wit, BaseStrat, AddStrat> StrategyStorageBuilder<Args, Wit, BaseStrat, AddStrat> {
    pub fn with_random(self) -> StrategyStorageBuilder<Args, Wit, Random<Args, Wit>, AddStrat> {
        StrategyStorageBuilder {
            base_strat: Some(Random::<Args, Wit>::default()),
            add_strat: self.add_strat,
            _placeholder: Default::default(),
        }
    }

    pub fn with_random_pool(self) -> StrategyStorageBuilder<Args, Wit, RandomValuePool<Args, Wit>, AddStrat> {
        StrategyStorageBuilder {
            base_strat: Some(RandomValuePool::<Args, Wit>::default()),
            add_strat: self.add_strat,
            _placeholder: Default::default(),
        }
    }

    pub fn with_custom_strategy<NewStrat>(
        self,
        custom_strat: NewStrat,
    ) -> StrategyStorageBuilder<Args, Wit, NewStrat, AddStrat>
    where
        NewStrat: Strategy<Value = (Arguments, WitnessValues)> + 'static,
    {
        StrategyStorageBuilder {
            base_strat: Some(custom_strat),
            add_strat: self.add_strat,
            _placeholder: Default::default(),
        }
    }

    pub fn with_additional_strategy<NewAddStrat>(
        self,
        strategy: NewAddStrat,
    ) -> StrategyStorageBuilder<Args, Wit, BaseStrat, NewAddStrat>
    where
        NewAddStrat: Strategy + 'static,
    {
        StrategyStorageBuilder {
            base_strat: self.base_strat,
            add_strat: Some(strategy),
            _placeholder: Default::default(),
        }
    }

    pub fn with_random_asset_value(self) -> StrategyStorageBuilder<Args, Wit, BaseStrat, std::ops::Range<u64>> {
        StrategyStorageBuilder {
            base_strat: self.base_strat,
            add_strat: Some(0..u64::MAX),
            _placeholder: Default::default(),
        }
    }
}

impl<Args, Wit, BaseStrat, AddStrat> StrategyStorageBuilder<Args, Wit, BaseStrat, AddStrat>
where
    BaseStrat: Strategy<Value = (Arguments, WitnessValues)> + 'static,
    AddStrat: Strategy + 'static,
{
    pub fn build(self) -> BoxedStrategy<((Arguments, WitnessValues), AddStrat::Value)> {
        let base = self
            .base_strat
            .expect("Base strategy is mandatory. Call with_random() or similar.");
        let add = self.add_strat.expect("Additional strategy is missing.");

        (base, add).boxed()
    }
}

impl<Program, Args, Wit, AdditionalValue> SimplexFuzzEngine<Program, Args, Wit, AdditionalValue>
where
    Program: FuzzableProgram<Program> + Clone + 'static,
    AdditionalValue: Clone + Debug + 'static,
{
    pub fn run_with_check(self, program_post_hook: impl ProgramCheck) {
        let mut runner = self.runner;
        let context = self.fuzz_context;
        let strategy = self.strategy_storage;
        let blueprint = self.blueprint;

        match runner.run(&strategy, |((args, wit), add)| {
            let ft = blueprint.get_final_tx(&context, &args, &wit, &add);
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
                    if let Err(e) = program_post_hook.call(&context, &pst, &args, &wit, i, exec_result) {
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
