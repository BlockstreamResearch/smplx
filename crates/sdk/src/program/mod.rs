pub mod arguments;
mod core;
pub mod error;
pub mod witness;

pub use arguments::ArgumentsTrait;
pub use core::{Program, ProgramTrait};
pub use error::ProgramError;
pub use witness::WitnessTrait;
