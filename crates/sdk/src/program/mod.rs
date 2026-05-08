/// Definitions and traits for handling program arguments in Simplicity programs.
pub mod arguments;
/// Core definitions, features, and abstractions for working with Simplicity programs.
pub mod core;
/// Error types and definitions for program compilation, manipulation, and execution failures.
pub mod error;
/// Definitions and traits for resolving and satisfying execution witnesses for Simplicity programs.
pub mod witness;

pub use arguments::ArgumentsTrait;
pub use core::{Program, ProgramTrait};
pub use error::ProgramError;
pub use simplicityhl::tracker::TrackerLogLevel;
pub use witness::WitnessTrait;
