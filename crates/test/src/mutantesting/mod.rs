pub mod core;
pub mod engine;
pub mod provider;
pub mod strategy;
pub mod utils;

pub use proptest::{
    prelude::*,
    strategy::NewTree,
    test_runner::{Config, TestRunner},
};

pub use core::{FuzzContext, FuzzableProgram, ProgramCheck, ProgramExecResult};
pub use engine::SimplexFuzzEngine;
pub use strategy::args::RandomValueTree;
pub use utils::{generate_value_by_ty, generate_value_by_ty_iterative};
