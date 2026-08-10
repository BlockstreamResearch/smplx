pub mod build;
pub mod dep_spec;
pub mod dependency;

pub use build::BuildConfig;
pub use dependency::{DEFAULT_DEPENDENCY_DIR, Dependency, DependencyConfig, GitRef};
