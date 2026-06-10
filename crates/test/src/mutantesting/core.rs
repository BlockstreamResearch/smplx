use crate::mutantesting::provider::MockProvider;
use proptest::prelude::BoxedStrategy;
use simplicityhl::elements::Script;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::simplicity::jet::Elements;
use simplicityhl::{Arguments, WitnessValues};
use smplx_sdk::program::ProgramError;
use smplx_sdk::program::core::SimplexProgram;
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::{Signer, SignerError};
use smplx_sdk::transaction::FinalTransaction;
use std::fmt::Debug;
use std::sync::Arc;

pub struct FuzzContext {
    pub signer: Option<Signer>,
    pub mock_provider: MockProvider,
    pub network: SimplicityNetwork,
}

use simplicityhl::simplicity::{RedeemNode, Value};

pub type ProgramExecResult = Result<(Arc<RedeemNode<Elements>>, Value), ProgramError>;
pub trait FuzzableProgram<P: SimplexProgram>: SimplexProgram {
    fn build_program(args: impl Into<Arguments>, network: &SimplicityNetwork) -> (Box<P>, Script);
}

pub trait FuzzableBaseContextGen<Program> {
    fn build_base_transaction(
        &self,
        network: &SimplicityNetwork,
        args: Arguments,
        wit: WitnessValues,
    ) -> FinalTransaction;
}

pub trait FuzzableContextGen<Program> {
    fn modify_transaction(
        &self,
        signer: &Option<Signer>,
        ft: FinalTransaction,
        args: &Arguments,
        wit: &WitnessValues,
    ) -> Result<PartiallySignedTransaction, SignerError>;
}

pub trait ArgGenFuzzStrategy<Args, Wit>: Debug {
    fn get_strategy(&self, test_context: &FuzzContext) -> BoxedStrategy<(Arguments, WitnessValues)>;
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
