use std::fmt::{Display, Formatter, Result as FmtResult};

use simplicityhl::simplicity::bit_machine::ExecutionError;

use smplx_sdk::program::ProgramError;

/// Result of executing one program input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Accepted,
    Rejected(Rejection),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rejection {
    /// A jet failed, e.g. `assert!`.
    Assertion,
    /// `panic!`.
    Panic,
    /// A pruned branch was reached, e.g. `unwrap(None)`.
    Unwrap,
    /// A Bit Machine limit exceeded.
    Resource,
    /// The program costs more than its witness pays for.
    Budget,
}

/// A broken fuzz setup. Not a statement about the contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupError {
    Scenario(String),
    Arguments(String),
    Witness(String),
    Transaction(String),
    Signing(String),
    Engine(String),
    Binding(String),
    Spec,
}

impl Rejection {
    pub const ALL: [Self; 5] = [Self::Assertion, Self::Panic, Self::Unwrap, Self::Resource, Self::Budget];
}

impl Outcome {
    /// # Errors
    /// Returns a [`SetupError`] when the error comes from the fuzz setup rather than the contract.
    pub fn classify<T>(result: Result<T, ProgramError>) -> Result<Self, SetupError> {
        let Err(error) = result else {
            return Ok(Self::Accepted);
        };

        let rejection = match &error {
            ProgramError::InsufficientBudget { .. } => Rejection::Budget,
            ProgramError::BitMachineCreation(_) => Rejection::Resource,
            ProgramError::Pruning(execution) | ProgramError::Execution(execution) => match execution {
                ExecutionError::JetFailed(_) => Rejection::Assertion,
                ExecutionError::ReachedFailNode(_) => Rejection::Panic,
                ExecutionError::ReachedPrunedBranch(_) => Rejection::Unwrap,
                ExecutionError::LimitExceeded(_) => Rejection::Resource,
                ExecutionError::InputWrongType(_) | ExecutionError::JetTypeMismatch => {
                    return Err(SetupError::Engine(error.to_string()));
                }
            },
            ProgramError::Compilation(_) | ProgramError::ProgramGenAbiMeta(_) => {
                return Err(SetupError::Arguments(error.to_string()));
            }
            ProgramError::WitnessSatisfaction(_) => return Err(SetupError::Witness(error.to_string())),
            ProgramError::UtxoIndexOutOfBounds { .. }
            | ProgramError::ScriptPubkeyMismatch { .. }
            | ProgramError::TxExtraction(_)
            | ProgramError::InputIndexOverflow(_) => return Err(SetupError::Transaction(error.to_string())),
        };

        Ok(Self::Rejected(rejection))
    }

    #[must_use]
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted)
    }
}

impl Display for Outcome {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Accepted => write!(f, "accepted"),
            Self::Rejected(rejection) => write!(f, "rejected ({rejection})"),
        }
    }
}

impl Display for Rejection {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let reason = match self {
            Self::Assertion => "assertion",
            Self::Panic => "panic",
            Self::Unwrap => "unwrap",
            Self::Resource => "resource limit",
            Self::Budget => "witness budget",
        };

        write!(f, "{reason}")
    }
}

impl Display for SetupError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Scenario(error) => write!(f, "the scenario is invalid: {error}"),
            Self::Arguments(error) => write!(f, "arguments do not fit the program: {error}"),
            Self::Witness(error) => write!(f, "witness does not fit the program: {error}"),
            Self::Transaction(error) => write!(f, "transaction does not fit the program: {error}"),
            Self::Signing(error) => write!(f, "signing failed: {error}"),
            Self::Engine(error) => write!(f, "program and environment disagree, please report a bug: {error}"),
            Self::Binding(name) => write!(f, "a binding of `{name}` panicked, see its message above"),
            Self::Spec => write!(f, "the spec panicked, see its message above"),
        }
    }
}

#[cfg(test)]
mod tests {
    use simplicityhl::{Arguments, WitnessValues};

    use smplx_sdk::program::{Program, ProgramTrait};
    use smplx_sdk::provider::SimplicityNetwork;
    use smplx_sdk::transaction::{FinalTransaction, PartialInput, RequiredSignature, UTXO};

    use super::*;

    fn finalize(source: &str) -> Result<Outcome, SetupError> {
        let network = SimplicityNetwork::default_regtest();
        let program = Program::new(source, Arguments::default());

        let mut utxo = UTXO::default();
        utxo.txout.script_pubkey = program.get_script_pubkey(&network);

        let mut transaction = FinalTransaction::new();
        transaction.add_input(PartialInput::new(utxo), RequiredSignature::None);

        let (pst, _) = transaction.extract_pst();

        Outcome::classify(program.finalize(&pst, &WitnessValues::default(), 0, &network))
    }

    #[test]
    fn classifies_executions() {
        assert_eq!(finalize("fn main() { assert!(true); }"), Ok(Outcome::Accepted));
        assert_eq!(
            finalize("fn main() { assert!(false); }"),
            Ok(Outcome::Rejected(Rejection::Assertion))
        );
        assert_eq!(
            finalize("fn main() { panic!(); }"),
            Ok(Outcome::Rejected(Rejection::Panic))
        );
        assert_eq!(
            finalize("fn main() { let x: Option<u8> = None; let y: u8 = unwrap(x); }"),
            Ok(Outcome::Rejected(Rejection::Unwrap))
        );
        assert!(matches!(
            finalize("fn main() { let w: u8 = witness::W; assert!(jet::eq_8(w, 0)); }"),
            Err(SetupError::Witness(_))
        ));
    }

    #[test]
    fn classifies_budget_as_rejection() {
        let error = ProgramError::InsufficientBudget {
            cost_wu: 2,
            budget_wu: 1,
            stack_bytes: 0,
            deficit_wu: 1,
        };

        assert_eq!(
            Outcome::classify::<()>(Err(error)),
            Ok(Outcome::Rejected(Rejection::Budget))
        );
    }
}
