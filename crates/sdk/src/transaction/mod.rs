/// Represents a fully finalized target transaction schema ready for signing and broadcasting.
pub mod final_transaction;
/// Represents inputs under construction before transaction finalization.
pub mod partial_input;
/// Represents outputs under construction before transaction finalization.
pub mod partial_output;
/// Contains data representing the submission status of a broadcast transaction.
pub mod tx_receipt;
/// Common representation of unspent transaction outputs used as funding sources.
pub mod utxo;

pub use final_transaction::{FinalInput, FinalTransaction, IssuanceDetails};
pub use partial_input::{PartialInput, ProgramInput, RequiredSignature};
pub use partial_output::PartialOutput;
pub use tx_receipt::TxReceipt;
pub use utxo::UTXO;
