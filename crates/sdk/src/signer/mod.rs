/// Core implementations and abstractions for transaction signing mechanisms in the Simplex SDK.
pub mod core;
/// Signer-specific error enumerations capturing execution constraints and mapping internal failure types.
pub mod error;
/// Contains abstractions that allow the `Signer` to remain agnostic to the origin of its keys.
mod key_origin;
/// Utilities for injecting witness data bindings into Simplicity environments.
mod wtns_injector;

pub use core::{Signer, SignerTrait};
pub use error::SignerError;
pub use key_origin::{HDKey, KeyOrigin, SingleKey};
