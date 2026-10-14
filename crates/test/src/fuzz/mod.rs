pub mod args_strategy;
pub mod core;
pub mod fuzz_program;
pub mod outcome;
pub mod scenario;
mod smart_check;
pub mod spec;
pub mod summary;
pub mod utils;

pub use proptest;

pub use core::SimplexFuzzEngine;
pub use fuzz_program::{FuzzProgram, FuzzProgramTrait};
pub use outcome::{Outcome, Rejection, SetupError};
pub use scenario::{FuzzProgramScenarioBuilder, Scenario};
pub use spec::{Expect, FuzzCase};
pub use utils::{generate_random_value_by_ty, random_arguments, random_witness};
