use std::sync::Arc;

use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::simplicity::bit_machine::ExecutionError;
use simplicityhl::simplicity::{RedeemNode, Value};
use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::ProgramError;
use smplx_sdk::signer::Signer;

pub type ProgramExecResult = Result<(Arc<RedeemNode>, Value), ProgramError>;

pub trait ProgramCheck<Program, Args, Wit> {
    fn call(
        &self,
        signer: &Signer,
        tx: &PartiallySignedTransaction,
        arguments: &Arguments,
        witness: &WitnessValues,
        input_index: usize,
        program_exec_result: ProgramExecResult,
    ) -> Result<(), String>;
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

/// Checks that each fuzzed program execution matches the expected outcome.
///
/// Used by `run`. For custom checks, implement [`ProgramCheck`]
/// and pass your check to `run_custom`.
pub struct FuzzExecutionCheck {
    test_name: &'static str,
    expect: Expect,
}

impl FuzzExecutionCheck {
    pub const fn new(test_name: &'static str, expect: Expect) -> Self {
        Self { test_name, expect }
    }

    fn expected_outcome(expect: Expect) -> &'static str {
        match expect {
            Expect::Ok => "execution succeeded",
            Expect::AssertFailed => "assertion failure (`assert!`)",
            Expect::PrunedBranch => "execution reached a pruned branch",
        }
    }
}

impl<Program, Args, Wit> ProgramCheck<Program, Args, Wit> for FuzzExecutionCheck {
    fn call(
        &self,
        _signer: &Signer,
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
