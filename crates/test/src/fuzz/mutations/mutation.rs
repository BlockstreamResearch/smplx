use std::fmt::{Display, Formatter, Result as FmtResult};

use simplicityhl::elements::{AssetId, LockTime, OutPoint, Script, Sequence, Txid};

use smplx_sdk::transaction::{FinalTransaction, PartialInput, PartialOutput, RequiredSignature, UTXO};

use crate::fuzz::outcome::SetupError;

/// One change to the assembled transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mutation {
    /// Transfers `amount` from output `from` to output `to`, which hold the same asset.
    TransferValue { from: usize, to: usize, amount: u64 },
    /// Pays output `output` in `asset`. An attacker input of `asset` and an attacker output
    /// taking the leftover amount are appended, so the transaction stays balanced.
    SwapAsset { output: usize, asset: AssetId },
    /// Sets the locktime of the transaction and of every input.
    SetLocktime(LockTime),
    /// Sets the sequence of input `input`.
    SetSequence { input: usize, sequence: Sequence },
}

impl Mutation {
    /// # Errors
    /// Returns a [`SetupError`] when the mutation does not fit `transaction`.
    pub(crate) fn apply(&self, transaction: &mut FinalTransaction) -> Result<(), SetupError> {
        let unfit = || SetupError::Engine(format!("`{self}` does not fit the transaction"));

        match *self {
            Self::TransferValue { from, to, amount } => {
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
            Self::SwapAsset { output, asset } => {
                let swapped = transaction.outputs().get(output).ok_or_else(unfit)?;
                let (amount, freed) = (swapped.amount, swapped.asset);

                if freed == asset {
                    return Err(unfit());
                }

                let vout = u32::try_from(transaction.n_inputs()).map_err(|_| unfit())?;
                let backing = UTXO::explicit(
                    OutPoint::new(Txid::from_byte_array([0xa7; 32]), vout),
                    Script::new(),
                    amount,
                    asset,
                );

                transaction.outputs_mut()[output].asset = asset;
                transaction.add_input(PartialInput::new(backing), RequiredSignature::None);
                transaction.add_output(PartialOutput::new(Self::attacker_script(), amount, freed));
            }
            Self::SetLocktime(locktime) => {
                for input in transaction.inputs_mut() {
                    input.partial_input.locktime = locktime;
                }

                transaction.set_locktime(locktime);
            }
            Self::SetSequence { input, sequence } => {
                transaction
                    .inputs_mut()
                    .get_mut(input)
                    .ok_or_else(unfit)?
                    .partial_input
                    .sequence = sequence;
            }
        }

        Ok(())
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::TransferValue { .. } => "transfer value",
            Self::SwapAsset { .. } => "swap asset",
            Self::SetLocktime(_) => "set locktime",
            Self::SetSequence { .. } => "set sequence",
        }
    }

    /// `OP_TRUE`, a script anyone can spend.
    fn attacker_script() -> Script {
        Script::from(vec![0x51])
    }
}

impl Display for Mutation {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::TransferValue { from, to, amount } => {
                write!(f, "transfer {amount} from output {from} to output {to}")
            }
            Self::SwapAsset { output, asset } => write!(f, "pay output {output} in asset {asset}"),
            Self::SetLocktime(locktime) => write!(f, "set the locktime to {}", locktime.to_consensus_u32()),
            Self::SetSequence { input, sequence } => {
                write!(
                    f,
                    "set the sequence of input {input} to {:#x}",
                    sequence.to_consensus_u32()
                )
            }
        }
    }
}
