use smplx_build::{ArtifactsGenerator, ArtifactsResolver, BuildConfig, DependencyConfig};

use crate::config::CONFIG_FILENAME;

use super::error::CommandError;

pub struct Build {}

impl Build {
    pub fn run(config: BuildConfig, deps: &DependencyConfig) -> Result<(), CommandError> {
        let output_dir = ArtifactsResolver::resolve_local_dir(&config.out_dir)?;
        let src_dir = ArtifactsResolver::resolve_local_dir(&config.src_dir)?;

        // NOTE: Assume that remappings already install
        let files_remapping = ArtifactsResolver::resolve_remappings(deps, CONFIG_FILENAME)?;

        dbg!(files_remapping.clone());

        // TODO: For all remappings need to check files_to_build and concatenate it to Vec<Path>
        let files_to_build = ArtifactsResolver::resolve_files_to_build(&config.src_dir, &config.simf_files)?;

        Ok(ArtifactsGenerator::generate_artifacts(
            &output_dir,
            &src_dir,
            &files_to_build,
            &files_remapping,
        )?)
    }
}
