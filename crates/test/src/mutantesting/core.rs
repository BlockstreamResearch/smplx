use simplicityhl::elements::Script;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::simplicity::{RedeemNode, Value};
use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::{Program, ProgramError, ProgramFactory};
use smplx_sdk::provider::{ProviderTrait, SimplicityNetwork};
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::FinalTransaction;

use std::sync::Arc;

#[derive(Clone)]
pub struct FuzzContext {
    pub signer: Arc<Option<Signer>>,
    pub mock_provider: Arc<dyn ProviderTrait>,
    pub network: SimplicityNetwork,
}

pub type ProgramExecResult = Result<(Arc<RedeemNode>, Value), ProgramError>;

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

pub trait ContractFuzzStrategy<Program, Args, Wit>: std::fmt::Debug {
    type AdditionalInput: std::fmt::Debug + 'static;

    fn gen_final_transaction(
        &self,
        test_context: FuzzContext,
        arguments: Arguments,
        witness: WitnessValues,
        additional: Self::AdditionalInput,
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
