use std::path::PathBuf;
use std::process::Stdio;

use simplex_build::{ArtifactsGenerator, ArtifactsResolver, BuildConfig};

use super::error::CommandError;

pub struct Build {}

impl Build {
    pub fn run(config: BuildConfig) -> Result<(), CommandError> {
        let output_dir = ArtifactsResolver::resolve_output_dir(&config.out_dir)?;
        let files_to_build = ArtifactsResolver::resolve_files_to_build(&config.src_dir, &config.simf_files)?;

        println!("{}", output_dir.to_str().unwrap());

        for file in files_to_build {
            println!("{}", file.to_str().unwrap());
        }

        Ok(())

        // Ok(ArtifactsGenerator::generate_artifacts(
        //     output_dir,
        //     files_to_build.as_slice(),
        // )?)

        // match build_config.only_files {
        //     true => {
        //         simplex_template_gen_core::expand_only_files(&cwd, &out_dir_unwrapped, &build_config.simf_files)?;
        //     }
        //     false => {
        //         simplex_template_gen_core::expand_files_with_nested_dirs(
        //             &cwd,
        //             &build_config.base_dir,
        //             &out_dir_unwrapped,
        //             &build_config.simf_files,
        //         )?;
        //     }
        // }
    }
}
