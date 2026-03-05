use std::env;
use std::path::{Path, PathBuf};

use globwalk::FileType;

use super::error::BuildError;

pub struct ArtifactsResolver {}

impl ArtifactsResolver {
    pub fn resolve_files_to_build(src_dir: &String, simfs: &Vec<String>) -> Result<Vec<PathBuf>, BuildError> {
        let cwd = env::current_dir()?;
        let base = cwd.join(src_dir);

        let mut paths = Vec::new();

        let walker = globwalk::GlobWalkerBuilder::from_patterns(base, simfs.as_slice())
            .follow_links(true)
            .file_type(FileType::FILE)
            .build()?
            .into_iter()
            .filter_map(Result::ok);

        for img in walker {
            paths.push(img.path().to_path_buf().canonicalize()?);
        }

        Ok(paths)
    }

    pub fn resolve_output_dir(path: &impl AsRef<Path>) -> Result<PathBuf, BuildError> {
        let mut path_outer = PathBuf::from(path.as_ref());

        if !path_outer.is_absolute() {
            let manifest_dir = env::current_dir()?;

            let mut path_local = PathBuf::from(manifest_dir);
            path_local.push(path_outer);

            path_outer = path_local;
        }

        if path_outer.extension().is_some() {
            return Err(BuildError::GenerationPath(format!(
                "Directories can't have an extension, path: '{}'",
                path_outer.display()
            )));
        }

        if path_outer.is_file() {
            return Err(BuildError::GenerationPath(format!(
                "Directory can't be a path, path: '{}'",
                path_outer.display()
            )));
        }

        let path_outer = path_outer.canonicalize()?;

        Ok(path_outer)
    }
}
