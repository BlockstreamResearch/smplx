//! Building Elements transactions for the [`Signer`](crate::signer::Signer) to complete.
//!
//! A [`FinalTransaction`] holds inputs and outputs. Each input spends a [`UTXO`] through a
//! [`PartialInput`], and an input that spends a Simplicity program's output also carries a
//! [`ProgramInput`] with the program and its witness. The signature the signer adds to each
//! input is a [`NativeSignature`] or a [`ProgramSignature`].
//!
//! ```ignore
//! let mut tx = FinalTransaction::new();
//!
//! tx.add_program_input(
//!     PartialInput::new(utxo),
//!     ProgramInput::new(Box::new(program), witness),
//!     Some(ProgramSignature::new("SIGNATURE")),
//! );
//! tx.add_output(PartialOutput::new(script, 40_000, network.policy_asset()));
//!
//! let receipt = signer.broadcast(&tx)?;
//! ```
//!
//! The signer adds the fee output, and a change output unless the change is too small to keep.
//! Don't add them yourself unless you sign with `Signer::sign_tx`, which signs exactly the
//! transaction it is given.

/// Represents outputs under construction before transaction finalization.
pub mod change_output;
/// Errors for transaction accounting and UTXO data access.
pub mod error;
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
pub use error::{FinalTransactionError, UtxoError};
pub use final_transaction::{FinalInput, FinalTransaction};
pub use partial_input::{IssuanceDetails, PartialInput, ProgramInput};
pub use partial_output::PartialOutput;
pub use required_signature::{NativeSignature, ProgramSignature, RequiredSignature, SigMessage};
pub use tx_receipt::TxReceipt;
pub use utxo::UTXO;
