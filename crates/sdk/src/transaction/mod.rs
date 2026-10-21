/// Represents outputs under construction before transaction finalization.
pub mod change_output;
/// Represents a fully finalized target transaction schema ready for signing and broadcasting.
pub mod final_transaction;
/// Represents inputs under construction before transaction finalization.
pub mod partial_input;
mod partial_output;
/// Describes the signatures the signer puts on inputs.
pub mod required_signature;
/// Contains data representing the submission status of a broadcast transaction.
pub mod tx_receipt;
/// Common representation of unspent transaction outputs used as funding sources.
pub mod utxo;

pub use change_output::ChangeOutput;
pub use final_transaction::{FinalInput, FinalTransaction};
pub use partial_input::{IssuanceDetails, PartialInput, ProgramInput};
pub use partial_output::PartialOutput;
pub use required_signature::{NativeSignature, ProgramSignature, RequiredSignature, SigMessage};
pub use tx_receipt::TxReceipt;
pub use utxo::UTXO;
