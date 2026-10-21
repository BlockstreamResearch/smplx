use std::fmt::{Display, Formatter, Result as FmtResult};

use simplicityhl::elements::{AssetId, LockTime, OutPoint, Script, Sequence, Txid};

use smplx_sdk::transaction::{FinalTransaction, PartialInput, PartialOutput, UTXO};

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
    /// Appends an attacker output with `amount` taken from output `from`.
    AddOutput { from: usize, amount: u64 },
    /// Appends an attacker input of `amount` of `asset` and an attacker output paying it out.
    AddInput { asset: AssetId, amount: u64 },
    /// Swaps two inputs.
    SwapInputs(usize, usize),
    /// Swaps two outputs.
    SwapOutputs(usize, usize),
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

                transaction.outputs_mut()[output].asset = asset;

                Self::add_attacker_utxos(transaction, asset, freed, amount);
            }
            Self::SetLocktime(locktime) => {
                for input in transaction.inputs_mut() {
                    input.partial_input.locktime = Some(locktime);
                }

                transaction.set_locktime(locktime);
            }
            Self::SetSequence { input, sequence } => {
                transaction
                    .inputs_mut()
                    .get_mut(input)
                    .ok_or_else(unfit)?
                    .partial_input
                    .sequence = Some(sequence);
            }
            Self::AddOutput { from, amount } => {
                let source = transaction.outputs_mut().get_mut(from).ok_or_else(unfit)?;
                source.amount = source.amount.checked_sub(amount).ok_or_else(unfit)?;
                let asset = source.asset;

                transaction.add_output(PartialOutput::new(Self::attacker_script(), amount, asset));
            }
            Self::AddInput { asset, amount } => {
                Self::add_attacker_utxos(transaction, asset, asset, amount);
            }
            Self::SwapInputs(first, second) => {
                if first == second || first.max(second) >= transaction.n_inputs() {
                    return Err(unfit());
                }

                transaction.inputs_mut().swap(first, second);
            }
            Self::SwapOutputs(first, second) => {
                if first == second || first.max(second) >= transaction.n_outputs() {
                    return Err(unfit());
                }

                transaction.outputs_mut().swap(first, second);
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
            Self::AddOutput { .. } => "add output",
            Self::AddInput { .. } => "add input",
            Self::SwapInputs(..) => "swap inputs",
            Self::SwapOutputs(..) => "swap outputs",
        }
    }

    /// Appends an attacker input of `amount` of `input_asset` and an attacker output of `amount` of `output_asset`.
    fn add_attacker_utxos(
        transaction: &mut FinalTransaction,
        input_asset: AssetId,
        output_asset: AssetId,
        amount: u64,
    ) {
        let utxo = UTXO::explicit(
            OutPoint::new(Txid::from_byte_array([0xa7; 32]), transaction.n_inputs() as u32),
            Script::new(),
            amount,
            input_asset,
        );

        transaction.add_input(PartialInput::new(utxo), None);
        transaction.add_output(PartialOutput::new(Self::attacker_script(), amount, output_asset));
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
            Self::AddOutput { from, amount } => write!(f, "add an output of {amount} from output {from}"),
            Self::AddInput { asset, amount } => write!(f, "add an input of {amount} in asset {asset}"),
            Self::SwapInputs(first, second) => write!(f, "swap inputs {first} and {second}"),
            Self::SwapOutputs(first, second) => write!(f, "swap outputs {first} and {second}"),
        }
    }
}
