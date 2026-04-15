use std::path::{Path, PathBuf};

pub use smplx_build_internal::error::BuildError;
use smplx_build_internal::{ArtifactsGenerator, ArtifactsResolver, BuildConfig};

#[allow(clippy::needless_doctest_main)]
/// Builder for configuring and running Simplex artifact generation from `build.rs`.
///
/// Settings are resolved in priority order (highest wins):
/// 1. Values set directly on the builder (`.src_dir(...)`, etc.)
/// 2. Values loaded from a `Simplex.toml` config file (`.config(...)`)
/// 3. Built-in defaults (`simf/`, `**/*.simf`, `src/artifacts`)
///
/// # Examples
///
/// No config file — configure everything in `build.rs`:
/// ```no_run
/// fn main() {
///     smplx_build::Builder::new()
///         .src_dir("simf")
///         .out_dir("src/artifacts")
///         .simf_files(["**/*.simf"])
///         .generate()
///         .unwrap();
/// }
/// ```
///
/// Load from `Simplex.toml` but override the output directory:
/// ```no_run
/// fn main() {
///     smplx_build::Builder::new()
///         .config("Simplex.toml")
///         .out_dir("src/generated")
///         .generate()
///         .unwrap();
/// }
/// ```
pub struct Builder {
    config_path: Option<PathBuf>,
    src_dir: Option<String>,
    out_dir: Option<String>,
    simf_files: Option<Vec<String>>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            config_path: None,
            src_dir: None,
            out_dir: None,
            simf_files: None,
        }
    }

    /// Load base settings from a `Simplex.toml` file.
    /// Fields set directly on the builder take precedence over values in the file.
    pub fn config(mut self, path: impl AsRef<Path>) -> Self {
        self.config_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Directory containing `.simf` source files (default: `simf`).
    pub fn src_dir(mut self, dir: impl Into<String>) -> Self {
        self.src_dir = Some(dir.into());
        self
    }

    /// Directory where generated Rust artifacts are written (default: `src/artifacts`).
    pub fn out_dir(mut self, dir: impl Into<String>) -> Self {
        self.out_dir = Some(dir.into());
        self
    }

    /// Glob patterns selecting which `.simf` files to compile (default: `["**/*.simf"]`).
    /// Replaces the full list — call once with all patterns you need.
    pub fn simf_files(mut self, patterns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.simf_files = Some(patterns.into_iter().map(Into::into).collect());
        self
    }

    /// Run artifact generation.
    ///
    /// Emits `cargo:rerun-if-changed` directives for the config file (if any)
    /// and every resolved `.simf` input file.
    pub fn generate(self) -> Result<(), BuildError> {
        // Start from defaults, then overlay the config file (if provided).
        let mut config = BuildConfig::default();

        if let Some(ref path) = self.config_path {
            if !path.exists() {
                return Err(BuildError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("config file not found: {}", path.display()),
                )));
            }
            config = BuildConfig::from_file(path)?;
            println!("cargo:rerun-if-changed={}", path.display());
        }

        // Builder fields take highest precedence.
        if let Some(src_dir) = self.src_dir {
            config.src_dir = src_dir;
        }
        if let Some(out_dir) = self.out_dir {
            config.out_dir = out_dir;
        }
        if let Some(simf_files) = self.simf_files {
            config.simf_files = simf_files;
        }

        let out_dir = ArtifactsResolver::resolve_local_dir(&config.out_dir)?;
        let src_dir = {
            let p = ArtifactsResolver::resolve_local_dir(&config.src_dir)?;
            // resolve_files_to_build canonicalizes paths (producing \\?\ UNC paths on
            // Windows), so canonicalize src_dir too so strip_prefix works correctly.
            if p.exists() { p.canonicalize()? } else { p }
        };
        let files = ArtifactsResolver::resolve_files_to_build(&config.src_dir, &config.simf_files)?;

        for file in &files {
            println!("cargo:rerun-if-changed={}", file.display());
        }

        ArtifactsGenerator::generate_artifacts(&out_dir, &src_dir, &files)?;

        Ok(())
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::needless_doctest_main)]
/// Generate Simplex contract artifacts from a `build.rs` script.
///
/// Reads the `[build]` section of `config_path` (a `Simplex.toml` file) and
/// generates Rust bindings for every `.simf` contract found. Equivalent to
/// `Builder::new().config(config_path).generate()`.
///
/// # Example
///
/// ```no_run
/// fn main() {
///     smplx_build::generate_artifacts("Simplex.toml").unwrap();
/// }
/// ```
pub fn generate_artifacts(config_path: impl AsRef<Path>) -> Result<(), BuildError> {
    Builder::new().config(config_path).generate()
}

/// Convenience macro for use in `build.rs`.
///
/// Calls [`generate_artifacts`] and panics with a clear message on failure.
///
/// ```no_run
/// // uses "Simplex.toml" by default
/// fn main() {
///     smplx_build::generate_artifacts!();
/// }
///
/// // explicit config path
/// fn main() {
///     smplx_build::generate_artifacts!("Simplex.toml");
/// }
/// ```
#[macro_export]
macro_rules! generate_artifacts {
    ($config:expr) => {
        if let Err(e) = $crate::generate_artifacts($config) {
            panic!("smplx-build: failed to generate artifacts: {}", e);
        }
    };
    () => {
        $crate::generate_artifacts!("Simplex.toml")
    };
}
