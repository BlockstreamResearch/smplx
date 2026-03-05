use serde::Deserialize;
use std::path::{Path, PathBuf};

use simplex_test::TestConfig;

use super::error::ConfigError;

pub const INIT_CONFIG: &str = include_str!("../../Simplex.default.toml");
pub const CONFIG_FILENAME: &str = "Simplex.toml";

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub test: Option<TestConfig>,
    #[serde(default)]
    pub build: Option<BuildConf>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildConf {
    pub compile_simf: Option<Vec<PathBuf>>,
    pub out_dir: Option<PathBuf>,
}

impl Config {
    pub fn get_default_path() -> Result<PathBuf, ConfigError> {
        let cwd = std::env::current_dir()?;

        Ok(cwd.join(CONFIG_FILENAME))
    }

    // TODO: load default values like `simf` path
    pub fn load(path_buf: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path_buf.as_ref().to_path_buf();

        if !path.is_file() {
            return Err(ConfigError::PathIsNotFile(path));
        }

        if !path.exists() {
            return Err(ConfigError::PathNotExists(path));
        }

        let conf_str = std::fs::read_to_string(path)?;

        Ok(toml::from_str(conf_str.as_str()).map_err(ConfigError::UnableToDeserialize)?)
    }
}

// fn resolve_glob_paths(pattern: &[impl AsRef<str>]) -> io::Result<Vec<PathBuf>> {
//     let mut paths = Vec::new();
//     for path in pattern.iter().map(|x| resolve_glob_path(x.as_ref())) {
//         let path = path?;
//         paths.extend_from_slice(&path);
//     }
//     Ok(paths)
// }

// fn resolve_glob_path(pattern: impl AsRef<str>) -> io::Result<Vec<PathBuf>> {
//     let mut paths = Vec::new();
//     for path in glob::glob(pattern.as_ref())
//         .map_err(|e| io::Error::other(e.to_string()))?
//         .filter_map(Result::ok)
//     {
//         println!("path: '{}', pattern: '{}'", path.display(), pattern.as_ref());
//         paths.push(path);
//     }
//     Ok(paths)
// }

// fn resolve_dir_path(path: impl AsRef<Path>) -> io::Result<PathBuf> {
//     let mut path_outer = PathBuf::from(path.as_ref());

//     if !path_outer.is_absolute() {
//         let manifest_dir = std::env::current_dir()?;

//         let mut path_local = PathBuf::from(manifest_dir);
//         path_local.push(path_outer);

//         path_outer = path_local;
//     }

//     if path_outer.extension().is_some() {
//         return Err(io::Error::other(format!(
//             "Folder can't have an extension, path: '{}'",
//             path_outer.display()
//         )));
//     }
//     if path_outer.is_file() {
//         return Err(io::Error::other(format!(
//             "Folder can't be a path, path: '{}'",
//             path_outer.display()
//         )));
//     }
//     Ok(path_outer)
// }
