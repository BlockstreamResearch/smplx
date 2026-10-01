use std::sync::Arc;

use simplicityhl::elements::Script;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::simplicity::{RedeemNode, Value};
use simplicityhl::{Arguments, WitnessNameToValueMap, WitnessValues};

use smplx_sdk::program::{Program, ProgramError, ProgramFactory};
use smplx_sdk::provider::SimplicityNetwork;

use crate::fuzz::engine::Context;

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

pub trait ProgramCheck<Program, Args, Wit> {
    fn call(
        &self,
        ctx: &Context,
        tx: &PartiallySignedTransaction,
        arguments: &Arguments,
        witness: &WitnessValues,
        input_index: usize,
        program_exec_result: ProgramExecResult,
    ) -> Result<(), String>;
}
