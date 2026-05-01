#![doc(html_logo_url = "https://raw.githubusercontent.com/BlockstreamResearch/smplx/master/assets/img/simplex_logo.jpg")]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/BlockstreamResearch/smplx/master/assets/img/simplex_logo.ico"
)]
#![doc(html_root_url = "https://docs.rs/smplx-std/latest/simplex/")]

#![cfg_attr(doc, doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR" ), "/", "README.md")))]
#![cfg_attr(not(doc), doc = "Simplex standard library")]
#![warn(clippy::all, clippy::pedantic)]

pub mod constants;
pub mod global;
pub mod program;
pub mod provider;
pub mod signer;
pub mod transaction;
pub mod utils;
