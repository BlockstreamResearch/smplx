use smplx_build::{ArtifactsGenerator, ArtifactsResolver, BuildConfig};

use super::error::CommandError;

pub struct Build {}

impl Build {
    /// Builds the project and generates artifacts based on the provided configuration.
    ///
    /// # Errors
    /// Returns a `CommandError` if it fails to resolve directories or files, or if artifact generation encounters an error.
    pub fn run(config: &BuildConfig) -> Result<(), CommandError> {
        let output_dir = ArtifactsResolver::resolve_local_dir(&config.out_dir)?;
        let src_dir = ArtifactsResolver::resolve_local_dir(&config.src_dir)?;
        let files_to_build = ArtifactsResolver::resolve_files_to_build(&config.src_dir, &config.simf_files)?;

        Ok(ArtifactsGenerator::generate_artifacts(
            &output_dir,
            &src_dir,
            &files_to_build,
        )?)
    }
}
