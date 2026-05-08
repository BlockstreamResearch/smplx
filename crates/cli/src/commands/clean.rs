use std::{fmt::Display, fs, path::PathBuf};

use smplx_build::{ArtifactsResolver, BuildConfig};

use crate::commands::error::CleanError;
use crate::commands::error::CommandError;

pub struct Clean;

pub struct DeletedItems(Vec<PathBuf>);

impl Clean {
    /// Cleans up generated artifacts from the project.
    ///
    /// This method removes compiled files and output directories based on the provided configuration.
    ///
    /// # Errors
    /// Returns a `CommandError` if it fails to resolve the artifacts directory or if an error occurs while removing the directories.
    pub fn run(config: &BuildConfig) -> Result<(), CommandError> {
        let deleted_files = Self::delete_files(config)?;

        println!("Deleted files: {deleted_files}");

        Ok(())
    }

    fn delete_files(config: &BuildConfig) -> Result<DeletedItems, CleanError> {
        let mut deleted_items = Vec::with_capacity(1);
        let generated_artifacts = Self::remove_artifacts(config)?;

        if let Some(artifacts_dir) = generated_artifacts {
            deleted_items.push(artifacts_dir);
        }

        Ok(DeletedItems(deleted_items))
    }

    fn remove_artifacts(config: &BuildConfig) -> Result<Option<PathBuf>, CleanError> {
        let output_dir = ArtifactsResolver::resolve_local_dir(&config.out_dir)
            .map_err(|e| CleanError::ResolveOutDir(e.to_string()))?;

        let res = if output_dir.exists() {
            fs::remove_dir_all(&output_dir).map_err(|e| CleanError::RemoveOutDir(e, output_dir.clone()))?;
            Some(output_dir)
        } else {
            None
        };

        Ok(res)
    }
}

impl Display for DeletedItems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write;

        let paths_len = self.0.len();
        let mut result = String::from("[");

        for (index, path) in self.0.iter().enumerate() {
            let _ = write!(result, "\n    {}", path.display());

            if index < paths_len - 1 {
                result.push(',');
            } else {
                result.push('\n');
            }
        }

        result.push(']');

        write!(f, "{result}")
    }
}
