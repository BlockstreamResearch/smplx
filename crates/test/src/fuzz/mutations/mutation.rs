use std::fmt::{Display, Formatter, Result as FmtResult};

use smplx_sdk::transaction::FinalTransaction;

use crate::fuzz::outcome::SetupError;

/// One change to the assembled transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mutation {
    /// Moves `amount` from output `from` to output `to`, which hold the same asset.
    MoveValue { from: usize, to: usize, amount: u64 },
}

impl Mutation {
    /// # Errors
    /// Returns a [`SetupError`] when the mutation does not fit `transaction`.
    pub(crate) fn apply(&self, transaction: &mut FinalTransaction) -> Result<(), SetupError> {
        let unfit = || SetupError::Engine(format!("`{self}` does not fit the transaction"));

        match *self {
            Self::MoveValue { from, to, amount } => {
                let outputs = transaction.outputs_mut();

                if from == to
                    || from >= outputs.len()
                    || to >= outputs.len()
                    || outputs[from].asset != outputs[to].asset
                {
                    return Err(unfit());
                }

                outputs[from].amount = outputs[from].amount.checked_sub(amount).ok_or_else(unfit)?;
                outputs[to].amount = outputs[to].amount.checked_add(amount).ok_or_else(unfit)?;
            }
        }

        Ok(())
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::MoveValue { .. } => "move value",
        }
    }
}

impl Display for Mutation {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::MoveValue { from, to, amount } => write!(f, "move {amount} from output {from} to output {to}"),
        }
    }
}
