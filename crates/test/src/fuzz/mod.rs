//! Property testing of Simplicity contracts, without a network.
//!
//! A fuzz test describes a transaction and the programs in it as a [`Scenario`], and states what
//! should happen to each spend. [`SimplexFuzzEngine`] then generates cases: for each, it draws
//! arguments, witnesses and storage for every program, optionally mutates the transaction, signs
//! it with the test's signer, and finalizes every program input locally, which checks the witness
//! budget too. It compares the result against the expectation and, for an accepted spend, runs
//! the lock and authorization checks.
//!
//! ```ignore
//! #[simplex::fuzz]
//! fn p2pk_fuzz(context: TestContext<FuzzMode>) {
//!     let public_key = context.get_default_signer().get_schnorr_public_key().serialize();
//!     let program = P2pkProgram::new(P2pkArguments { public_key }).as_ref().clone();
//!
//!     let mut transaction = FinalTransaction::new();
//!     transaction.add_program_input(
//!         PartialInput::default(),
//!         ProgramInput::new(Box::new(program), P2pkWitness::default()),
//!         Some(ProgramSignature::new("SIGNATURE")),
//!     );
//!
//!     let mut scenario = Scenario::new(transaction);
//!     scenario
//!         .add::<P2pkProgram>("p2pk")
//!         .spends(0)
//!         .bind_arguments(move |arguments, _| arguments.public_key = public_key);
//!
//!     context.engine(scenario).run();
//! }
//! ```
//!
//! A run fails with one of these findings:
//!
//! - **unsound**: the contract accepted a spend that the spec rejects, or that violates a
//!   witness requirement.
//! - **liveness**: the contract rejected a spend that the spec accepts.
//! - **lock bypass**: an accepted spend is still accepted after an output the program locks is
//!   redirected to an attacker.
//! - **authorization bypass**: an accepted spend is still accepted with an attacker's
//!   signature, or with a signature made for another transaction.
//!
//! It also fails if no case was accepted, if the spec returned [`Expect::Any`] for every case, or
//! if the setup is broken, for example if the programs don't fit the transaction. A broken setup
//! says so in its message, and doesn't mean anything about the contract.
//!
//! Failing cases aren't shrunk. The number of cases, the seed and the network come from the
//! `[test.fuzz]` section of `Simplex.toml`.

mod case;
mod common;
mod core;
mod execution;
mod mutations;
mod outcome;
mod program;
mod program_scenario;
mod program_strategy;
mod requirement;
mod scenario;
mod scenario_strategy;
mod smart_check;
mod summary;
mod utils;
mod witness_path;

pub use proptest;

pub use case::{Expect, FuzzCase};
pub use core::SimplexFuzzEngine;
pub use mutations::{Mutation, Mutations};
pub use program::{FuzzProgram, FuzzProgramTrait};
pub use program_scenario::FuzzProgramScenarioBuilder;
pub use program_strategy::FuzzProgramValues;
pub use scenario::Scenario;
pub use utils::{generate_random_value_by_ty, random_arguments, random_witness};
