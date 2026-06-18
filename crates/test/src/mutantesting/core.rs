use proptest::prelude::{BoxedStrategy, Strategy};
use simplicityhl::elements::Script;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::simplicity::{RedeemNode, Value};
use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::{Program, ProgramError, ProgramFactory, RandomArguments, RandomWitness};
use smplx_sdk::provider::{ProviderTrait, SimplicityNetwork};
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::FinalTransaction;

use std::fmt::Debug;
use std::sync::Arc;

#[derive(Clone)]
pub struct FuzzContext {
    pub signer: Arc<Option<Signer>>,
    pub mock_provider: Arc<dyn ProviderTrait>,
    pub network: SimplicityNetwork,
}

pub type ProgramExecResult = Result<(Arc<RedeemNode>, Value), ProgramError>;

pub type GenStrategyExt<Program, Args, Wit, T> = (
    BoxedStrategy<((Arguments, WitnessValues), T)>,
    Box<dyn UserFuzzStrategyExt<Program, Args, Wit, T>>,
);

pub type GenStrategy<Program, Args, Wit> = (
    BoxedStrategy<(Arguments, WitnessValues)>,
    Box<dyn UserFuzzStrategy<Program, Args, Wit>>,
);

pub trait FuzzableProgram<P: AsRef<Program>>: AsRef<Program> {
    fn build_program(args: impl Into<Arguments>, network: &SimplicityNetwork) -> (Box<P>, Script);
}

impl<P: AsRef<Program> + ProgramFactory<P>> FuzzableProgram<P> for P {
    fn build_program(args: impl Into<Arguments>, network: &SimplicityNetwork) -> (Box<P>, Script) {
        let prog = P::instantiate_program(args);
        let script = prog.as_ref().as_ref().get_script_pubkey(network);
        (prog, script)
    }
}

pub trait FuzzStrategy<Program, Args, Wit>: Debug {
    fn get_strategy(self, test_context: FuzzContext) -> BoxedStrategy<(Arguments, WitnessValues, FinalTransaction)>;
}

impl<Program: 'static, Args: RandomArguments + 'static, Wit: RandomWitness + 'static> FuzzStrategy<Program, Args, Wit>
    for (
        BoxedStrategy<(Arguments, WitnessValues)>,
        Box<dyn UserFuzzStrategy<Program, Args, Wit>>,
    )
{
    fn get_strategy(self, test_context: FuzzContext) -> BoxedStrategy<(Arguments, WitnessValues, FinalTransaction)> {
        let (init_strategy, pst_strategy) = self;

        let result_strategy = init_strategy
            .prop_map(move |(args, wit)| pst_strategy.gen_final_transaction(test_context.clone(), args, wit));

        result_strategy.boxed()
    }
}

impl<Program: 'static, Args: RandomArguments + 'static, Wit: RandomWitness + 'static, T: Debug + 'static>
    FuzzStrategy<Program, Args, Wit>
    for (
        BoxedStrategy<((Arguments, WitnessValues), T)>,
        Box<dyn UserFuzzStrategyExt<Program, Args, Wit, T>>,
    )
{
    fn get_strategy(self, test_context: FuzzContext) -> BoxedStrategy<(Arguments, WitnessValues, FinalTransaction)> {
        let (init_strategy, pst_strategy) = self;

        let result_strategy = init_strategy.prop_map(move |((args, wit), additional_value)| {
            pst_strategy.gen_final_transaction(test_context.clone(), args, wit, additional_value)
        });

        result_strategy.boxed()
    }
}

pub trait UserFuzzStrategy<Program, Args, Wit>: Debug {
    fn gen_final_transaction(
        &self,
        test_context: FuzzContext,
        arguments: Arguments,
        witness: WitnessValues,
    ) -> (Arguments, WitnessValues, FinalTransaction);
}

pub trait UserFuzzStrategyExt<Program, Args, Wit, T>: Debug {
    fn gen_final_transaction(
        &self,
        test_context: FuzzContext,
        arguments: Arguments,
        witness: WitnessValues,
        additional_value: T,
    ) -> (Arguments, WitnessValues, FinalTransaction);
}

pub trait ProgramCheck {
    fn call(
        &self,
        ctx: &FuzzContext,
        tx: &PartiallySignedTransaction,
        arguments: &Arguments,
        witness: &WitnessValues,
        input_index: usize,
        program_exec_result: ProgramExecResult,
    ) -> Result<(), String>;
}
