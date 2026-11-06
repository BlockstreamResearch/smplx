#![warn(clippy::all, clippy::pedantic, missing_docs)]
//! WebAssembly bindings for the Simplex SDK.
//!
//! This crate exists so the SDK itself stays free of `wasm-bindgen` annotations
//! and follows the arrangement of `lwk_wasm`.

mod issuance_details;
mod network;
mod program;
mod signed_transaction;
mod signer;
mod transaction_builder;
mod version;

pub use issuance_details::IssuanceDetails;
pub use program::{Program, program_argument_types};
pub use signed_transaction::SignedTransaction;
pub use signer::Signer;
pub use transaction_builder::TransactionBuilder;
pub use version::sdk_version;
