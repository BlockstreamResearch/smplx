use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::path::{Path, PathBuf};
use std::{env, io};

use globwalk::FileType;
use serde::{Deserialize, Serialize};
use simplicityhl::resolution::{CanonPath, DependencyMap};

use crate::{BuildConfig, DependencyConfig};

use super::error::BuildError;

pub struct ArtifactsResolver {}

impl ArtifactsResolver {
    // Here need to load all files, include the remappings
    pub fn resolve_files_to_build(src_dir: &String, simfs: &[String]) -> Result<Vec<PathBuf>, BuildError> {
        let cwd = env::current_dir()?;
        let base = cwd.join(src_dir);

        let mut paths = Vec::new();

        let walker = globwalk::GlobWalkerBuilder::from_patterns(base, simfs)
            .follow_links(true)
            .file_type(FileType::FILE)
            .build()?
            .filter_map(Result::ok);

        for img in walker {
            paths.push(img.path().to_path_buf().canonicalize()?);
        }

        // Resolve here
        // NOTE!!! Filter out files without main function

        Ok(paths)
    }

    pub fn resolve_local_dir(path: &impl AsRef<Path>) -> Result<PathBuf, BuildError> {
        let mut path_outer = PathBuf::from(path.as_ref());

        if !path_outer.is_absolute() {
            let manifest_dir = env::current_dir()?;

            let mut path_local = manifest_dir;
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

        // TODO: canonicalize? but this path may not exist
        Ok(path_outer)
    }

    /// Converts "https://github.com/smplx/core.git" 
    /// into a Cargo-style path: "core-a1b2c3d4e5f67890"
    pub fn generate_hashed_repo_path(url: &str) -> Option<PathBuf> {
        let clean_url = url.strip_suffix(".git").unwrap_or(url);
        let repo_name = clean_url.split('/').last()?;

        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let hash_value = hasher.finish();

        // Do it the Rust way: EXACTLY 16 hex characters
        let folder_name = format!("{}-{:016x}", repo_name, hash_value);

        Some(PathBuf::from(folder_name))
    }

    // Key: dependency alias
    // Value: (path to dependency, git repo of the dependency)
    // TODO: Add git support (hardcoded for now)
    // Get rid of `unwrap`
    pub fn resolve_remappings(
        deps_config: &DependencyConfig,
        config_filename: &str,
    ) -> Result<JsonDependencyMap, BuildError> {
        let mut deps_map = JsonDependencyMap::default();
        //let src_dir = Path::new(src_dir);
        let root_dir = &env::current_dir()?;

        // let canon_root = CanonPath::canonicalize(root_dir)
        //     .map_err(BuildError::PathCanonicalization)?;

        for (drp_name, dep) in deps_config.inner.iter() {
            let dep_path = root_dir.join(dep.path.as_ref().unwrap());
            deps_map.insert(root_dir, drp_name, &dep_path);

            // let canon_dep_path = CanonPath::canonicalize(&dep_path)
            //     .map_err(BuildError::PathCanonicalization)?;
            //
            // deps_map.insert(canon_root.clone(), drp_name.clone(), canon_dep_path.clone())?;
            // Self::resolve_inner_remappings(&mut deps_map, canon_dep_path, config_filename)?;
        }

        Ok(deps_map)
    }

    fn resolve_inner_remappings(
        deps_map: &mut DependencyMap,
        canon_root: CanonPath,
        config_filename: &str,
    ) -> Result<(), BuildError> {
        let toml = &canon_root.as_path().join(config_filename);
        let deps_config = DependencyConfig::from_file(toml)?;
        let build_config = BuildConfig::from_file(toml)?;
        let src_dir = canon_root.as_path().join(&build_config.src_dir);

        for (drp_name, dep) in deps_config.inner.iter() {
            let dep_path = src_dir.join(dep.path.as_ref().unwrap());
            let canon_dep_path = CanonPath::canonicalize(&dep_path).map_err(BuildError::PathCanonicalization)?;

            deps_map.insert(canon_root.clone(), drp_name.clone(), canon_dep_path.clone())?;
            Self::resolve_inner_remappings(deps_map, canon_dep_path, config_filename)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JsonRemapping {
    pub context_prefix: PathBuf,
    pub drp_name: String,
    pub target: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JsonDependencyMap {
    pub remappings: Vec<JsonRemapping>,
}

impl JsonDependencyMap {
    pub fn insert(&mut self, context: &Path, drp_name: &str, target: &Path) {
        self.remappings.push(JsonRemapping {
            context_prefix: context.to_path_buf(),
            drp_name: drp_name.to_owned(),
            target: target.to_path_buf(),
        });
    }
}

impl TryFrom<JsonDependencyMap> for DependencyMap {
    type Error = BuildError;

    fn try_from(json_map: JsonDependencyMap) -> Result<Self, Self::Error> {
        let mut deps_map = DependencyMap::new();

        for json_remap in json_map.remappings {
            let context =
                CanonPath::canonicalize(&json_remap.context_prefix).map_err(BuildError::PathCanonicalization)?;
            let target = CanonPath::canonicalize(&json_remap.target).map_err(BuildError::PathCanonicalization)?;

            deps_map.insert(context, json_remap.drp_name, target)?;
        }

        Ok(deps_map)
    }
}

// TODO: Think about better error, because it will be used directly in examples
pub fn load_dependency_map(json_content: &str) -> Result<DependencyMap, BuildError> {
    let raw_data: JsonDependencyMap =
        serde_json::from_str(json_content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    raw_data.try_into()
}
