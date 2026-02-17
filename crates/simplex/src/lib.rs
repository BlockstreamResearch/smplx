#![warn(clippy::all, clippy::pedantic)]

//! High-level helpers for building and executing Simplicity programs on Liquid.

pub extern crate either;
pub extern crate serde;

#[cfg(feature = "macros")]
pub extern crate simplex_macros;

#[cfg(feature = "core")]
pub extern crate simplex_core;

#[cfg(feature = "macros")]
pub extern crate simplex_test;

#[cfg(feature = "macros")]
pub extern crate simplex_explorer;
