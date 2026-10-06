use std::sync::Arc;

use simplicityhl::elements::Script;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::simplicity::bit_machine::ExecutionError;
use simplicityhl::simplicity::{RedeemNode, Value};
use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::{Program, ProgramError, ProgramFactory};
use smplx_sdk::provider::SimplicityNetwork;

use crate::fuzz::core::FuzzContext;

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
        ctx: &FuzzContext,
        tx: &PartiallySignedTransaction,
        arguments: &Arguments,
        witness: &WitnessValues,
        input_index: usize,
        program_exec_result: ProgramExecResult,
    ) -> Result<(), String>;
}

/// Checks that each fuzzed program execution matches the expected outcome.
///
/// Used by `run_with_default_check`. For custom checks, implement [`ProgramCheck`]
/// and pass your check to `run_with_check`.
pub struct FuzzExecutionCheck {
    test_name: &'static str,
    expect: Expect,
}

#[derive(Clone, Copy)]
pub enum Expect {
    /// Program execution succeeds.
    Ok,
    /// A jet fails during execution, for example because a contract `assert!` fails.
    AssertFailed,
    /// Execution reaches a pruned branch, for example through `unwrap(None)` or a `safe_*` overflow.
    PrunedBranch,
}

impl FuzzExecutionCheck {
    pub const fn new(test_name: &'static str, expect: Expect) -> Self {
        Self { test_name, expect }
    }

    fn expected_outcome(expect: Expect) -> &'static str {
        match expect {
            Expect::Ok => "execution to succeed",
            Expect::AssertFailed => "an assertion failure (`assert!`)",
            Expect::PrunedBranch => "execution to reach a pruned branch",
        }
    }
}

impl<Program, Args, Wit> ProgramCheck<Program, Args, Wit> for FuzzExecutionCheck {
    fn call(
        &self,
        _context: &FuzzContext,
        _transaction: &PartiallySignedTransaction,
        _arguments: &Arguments,
        _witness: &WitnessValues,
        _input_index: usize,
        program_exec_result: ProgramExecResult,
    ) -> Result<(), String> {
        match (self.expect, &program_exec_result) {
            (Expect::Ok, Ok(_)) => Ok(()),
            (Expect::AssertFailed, Err(ProgramError::Pruning(ExecutionError::JetFailed(_)))) => Ok(()),
            (Expect::PrunedBranch, Err(ProgramError::Pruning(ExecutionError::ReachedPrunedBranch(_)))) => Ok(()),
            (expect, Ok(_)) => Err(format!(
                "Fuzz test `{}`: expected {}, but execution succeeded",
                self.test_name,
                Self::expected_outcome(expect),
            )),
            (expect, Err(error)) => Err(format!(
                "Fuzz test `{}`: expected {}, but execution failed: {error}",
                self.test_name,
                Self::expected_outcome(expect),
            )),
        }
    }
}
