pub mod args_strategy;
mod authorization;
pub mod core;
pub mod fuzz_program;
pub mod fuzz_transaction;
pub mod outcome;
pub mod spec;
pub mod summary;
pub mod utils;

pub use proptest;

pub use crate::error::FuzzError;
pub use core::SimplexFuzzEngine;
pub use fuzz_program::FuzzableProgram;
pub use outcome::{Outcome, Rejection, SetupError};
pub use spec::{Expect, FuzzCase};
pub use utils::{generate_random_value_by_ty, random_arguments, random_witness};
