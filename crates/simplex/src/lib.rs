#![warn(clippy::all, clippy::pedantic)]

pub extern crate either;
pub extern crate serde;

#[cfg(feature = "macros")]
pub extern crate simplex_macros;

#[cfg(feature = "sdk")]
pub extern crate simplex_sdk;

#[cfg(feature = "macros")]
pub extern crate simplex_test;

#[cfg(feature = "macros")]
pub extern crate tracing;

#[cfg(feature = "encoding")]
pub extern crate bincode;
