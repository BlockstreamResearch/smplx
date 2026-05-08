/// Represents a fully finalised target transaction ready for broadcast or analysis.
pub mod final_transaction;
/// Represents inputs under construction or execution requirements prior to finalisation.
pub mod partial_input;
/// Represents outputs under construction prior to transaction finalisation.
pub mod partial_output;
/// Contains data representing the submission status and outcome of a broadcasted transaction.
pub mod tx_receipt;
/// Common references mapping unspent transaction outputs used as funding sources.
pub mod utxo;

pub use final_transaction::{FinalInput, FinalTransaction, IssuanceDetails};
pub use partial_input::{PartialInput, ProgramInput, RequiredSignature};
pub use partial_output::PartialOutput;
pub use tx_receipt::TxReceipt;
pub use utxo::UTXO;
