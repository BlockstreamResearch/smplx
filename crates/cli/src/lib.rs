#![warn(clippy::all, clippy::pedantic)]

pub mod cli;
pub mod commands;
pub mod config;
pub mod error;

pub use cli::Cli;
