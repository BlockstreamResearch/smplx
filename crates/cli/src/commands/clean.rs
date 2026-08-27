use std::path::Path;
use std::{fmt::Display, fs, path::PathBuf};

use smplx_build::ArtifactsResolver;
use smplx_build::config::DEFAULT_DEPENDENCY_DIR;

use crate::commands::CleanFlags;
use crate::commands::error::CleanError;
use crate::commands::error::CommandError;

pub struct Clean;

pub struct DeletedItems(Vec<PathBuf>);

impl Clean {
    /// Cleans up generated artifacts from the project.
    ///
    /// Resolves which directories to remove based on `flags` and deletes them.
    ///
    /// # Errors
    /// Returns a `CommandError` if the artifacts directory cannot be resolved
    /// or if removing any directory fails.
    pub fn run(artifacts: &impl AsRef<Path>, flags: &CleanFlags) -> Result<(), CommandError> {
        let artifacts_dir = ArtifactsResolver::resolve_local_dir(artifacts)
            .map_err(|err| CleanError::ResolveOutDir(err.to_string()))?;

        let mut to_remove = vec![artifacts_dir];
        if flags.remove_all {
            let deps_dir = ArtifactsResolver::resolve_local_dir(&DEFAULT_DEPENDENCY_DIR)
                .map_err(|err| CleanError::ResolveOutDir(err.to_string()))?;

            to_remove.push(deps_dir);
        }

        let deleted = Self::delete_files(&to_remove)?;
        println!("Deleted files: {deleted}");

        Ok(())
    }

    fn delete_files(paths: &[PathBuf]) -> Result<DeletedItems, CleanError> {
        let mut deleted = Vec::with_capacity(paths.len());

        for path in paths {
            if !path.exists() {
                continue;
            }

            fs::remove_dir_all(path).map_err(|e| CleanError::RemoveOutDir(e, path.clone()))?;
            deleted.push(path.clone());
        }

        Ok(DeletedItems(deleted))
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
