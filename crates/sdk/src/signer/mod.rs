/// Core implementations and abstractions for transaction signing mechanisms in the Simplex SDK.
pub mod core;
/// Signer-specific error enumerations capturing execution constraints and mapping internal failure types.
pub mod error;
/// Utilities for injecting witness data bindings into Simplicity environments.
mod wtns_injector;

pub use core::{HDKey, KeyOrigin, Signer, SignerTrait, SingleKey};
pub use error::SignerError;
