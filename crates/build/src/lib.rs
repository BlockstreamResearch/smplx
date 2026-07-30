mod collector;
pub mod config;
pub mod dep_spec;
pub mod error;
pub mod generator;
pub mod macros;
pub mod resolver;

pub use config::{BuildConfig, DependencyConfig};
pub use generator::ArtifactsGenerator;
pub use resolver::ArtifactsResolver;
