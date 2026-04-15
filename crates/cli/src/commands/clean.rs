use std::{fmt::Display, fs, path::PathBuf};

use smplx_build_internal::{ArtifactsResolver, BuildConfig};

use crate::commands::error::CleanError;
use crate::commands::error::CommandError;

pub struct Clean;

pub struct DeletedItems(Vec<PathBuf>);

impl Clean {
    pub fn run(config: BuildConfig) -> Result<(), CommandError> {
        let deleted_files = Self::delete_files(config)?;

        println!("Deleted files: {deleted_files}");

        Ok(())
    }

    fn delete_files(config: BuildConfig) -> Result<DeletedItems, CleanError> {
        let mut deleted_items = Vec::with_capacity(1);
        let generated_artifacts = Self::remove_artifacts(config)?;

        if let Some(artifacts_dir) = generated_artifacts {
            deleted_items.push(artifacts_dir);
        }

        Ok(DeletedItems(deleted_items))
    }

    fn remove_artifacts(config: BuildConfig) -> Result<Option<PathBuf>, CleanError> {
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
}

impl Display for DeletedItems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let paths_len = self.0.len();
        let mut result = String::from("[");

        for (index, path) in self.0.iter().enumerate() {
            result.push_str(&format!("\n    {}", path.display()));

            if index < paths_len - 1 {
                result.push(',');
            } else {
                result.push('\n');
            }
        }

        result.push(']');

        write!(f, "{}", result)
    }
}
