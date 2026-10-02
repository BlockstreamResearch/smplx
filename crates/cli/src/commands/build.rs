use std::path::Path;

use smplx_build::{ArtifactsGenerator, ArtifactsResolver, BuildConfig, DependencyConfig};

use super::error::CommandError;

pub struct Build {}

impl Build {
    /// Builds the project and generates artifacts based on the provided configuration.
    /// Relative `out_dir` and `src_dir` paths are resolved against `root_dir`.
    ///
    /// # Errors
    /// Returns a `CommandError` if it fails to resolve directories or files, or if artifact generation encounters an error.
    pub fn run(root_dir: &Path, config: &BuildConfig, deps: &DependencyConfig) -> Result<(), CommandError> {
        let output_dir = ArtifactsResolver::resolve_local_dir(root_dir, &config.out_dir)?;
        let src_dir = ArtifactsResolver::resolve_local_dir(root_dir, &config.src_dir)?;

        // NOTE: Assumes that remappings are already installed
        let dependency_builder = ArtifactsResolver::resolve_remappings(root_dir, deps)?;

        let files_to_build = ArtifactsResolver::resolve_files_to_build(root_dir, &config.src_dir, &config.simf_files)?;

        Ok(ArtifactsGenerator::generate_artifacts(
            root_dir,
            &output_dir,
            &src_dir,
            &files_to_build,
            &dependency_builder,
        )?)
    }
}
