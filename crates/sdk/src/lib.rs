#![doc(html_logo_url = "https://raw.githubusercontent.com/BlockstreamResearch/smplx/master/docs/simplex_logo.png")]
#![doc(html_root_url = "https://docs.rs/smplx-sdk/latest/simplex/")]
#![cfg_attr(doc, doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR" ), "/", "README.md")))]
#![cfg_attr(not(doc), doc = "Simplex SDK")]
#![warn(clippy::all, clippy::pedantic, missing_docs)]

/// Common constants and identifiers used across the Simplex SDK.
pub mod constants;
/// Global state, configuration, and shared context used throughout the SDK.
pub mod global;
/// Core abstractions, definitions, and errors for compiling and evaluating Simplicity programs.
pub mod program;
/// Interfaces and implementations for interacting with blockchain nodes and APIs.
pub mod provider;
/// Traits and mechanisms for signing transactions and satisfying witness requirements.
pub mod signer;
/// Constructs and builders for assembling, tracking, and managing Elements transactions.
pub mod transaction;
/// General utility functions, conversions, and helper tools.
pub mod utils;
