use crate::mutantesting::provider::MockProvider;
use proptest::prelude::BoxedStrategy;
use simplicityhl::elements::Script;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::{Arguments, WitnessValues};
use smplx_sdk::program::ProgramError;
use smplx_sdk::program::core::SimplexProgram;
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Clone)]
pub struct FuzzContext {
    pub signer: Arc<Option<Signer>>,
    pub mock_provider: Arc<MockProvider>,
    pub network: SimplicityNetwork,
}

use simplicityhl::simplicity::{RedeemNode, Value};

pub type ProgramExecResult = Result<(Arc<RedeemNode>, Value), ProgramError>;
pub trait FuzzableProgram<P: SimplexProgram>: SimplexProgram {
    fn build_program(args: impl Into<Arguments>, network: &SimplicityNetwork) -> (Box<P>, Script);
}

pub trait FuzzStrategy<Program, Args, Wit>: Debug {
    fn get_strategy(
        &self,
        test_context: FuzzContext,
    ) -> BoxedStrategy<(Arguments, WitnessValues, PartiallySignedTransaction)>;
}

pub trait ProgramCheck {
    fn call(
        &self,
        ctx: &FuzzContext,
        tx: &PartiallySignedTransaction,
        arguments: &Arguments,
        witness: &WitnessValues,
        program_exec_result: ProgramExecResult,
    ) -> Result<(), String>;
}
