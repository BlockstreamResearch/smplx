use crate::commands::CleanFlags;
use crate::commands::error::{CleanError, CleanResult, CommandResult};
use smplx_build::{ArtifactsResolver, BuildConfig};
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Clean;

pub struct DeletedItems(Vec<PathBuf>);

impl Clean {
    pub fn run(config: BuildConfig, additional_flags: CleanFlags, config_path: impl AsRef<Path>) -> CommandResult<()> {
        let deleted_files = Self::delete_files(config, additional_flags, config_path)?;
        println!("Deleted files: {deleted_files}");
        Ok(())
    }
}

impl Clean {
    fn delete_files(
        config: BuildConfig,
        additional_flags: CleanFlags,
        smplx_toml_path: impl AsRef<Path>,
    ) -> CleanResult<DeletedItems> {
        let mut deleted_items = Vec::with_capacity(2);

        let generated_artifacts = Self::remove_artifacts(config)?;
        if let Some(artifacts_dir) = generated_artifacts {
            deleted_items.push(artifacts_dir);
        }
        if additional_flags.all {
            let simplex_toml_path = Self::remove_file(smplx_toml_path)?;
            if let Some(simplex_toml) = simplex_toml_path {
                deleted_items.push(simplex_toml);
            }
        }
        Ok(DeletedItems(deleted_items))
    }

    fn remove_artifacts(config: BuildConfig) -> CleanResult<Option<PathBuf>> {
        let output_dir = ArtifactsResolver::resolve_local_dir(&config.out_dir)
            .map_err(|e| CleanError::ResolveOutDir(e.to_string()))?;
        let res = if output_dir.exists() {
            fs::remove_dir_all(&output_dir).map_err(|e| CleanError::RemoveOutDir(e, output_dir.to_path_buf()))?;
            Some(output_dir)
        } else {
            None
        };
        Ok(res)
    }

    fn remove_file(config_path: impl AsRef<Path>) -> CleanResult<Option<PathBuf>> {
        let config_path = config_path.as_ref().to_path_buf();
        if config_path.exists() && config_path.is_file() {
            fs::remove_file(&config_path).map_err(|e| CleanError::RemoveFile(e, config_path.to_path_buf()))?;
            Ok(Some(config_path))
        } else {
            Ok(None)
        }
    }
}

impl Display for DeletedItems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let paths_len = self.0.len();
        let mut result = String::from("[");
        for (index, path) in self.0.iter().enumerate() {
            result.push_str(&format!("\n\t{}", path.display()));
            if index < paths_len - 1 {
                result.push(',');
            }
            result.push('\n');
        }
        result.push(']');
        write!(f, "{}", result)
    }
}
