use serde::{Deserialize, Serialize};
use simplex_sdk::constants::SimplicityNetwork;
use simplex_test::{ElementsDConf, RpcCreds};
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const DEFAULT_CONFIG: &str = include_str!("../../../Simplex.example.toml");
const CONFIG_FILENAME: &str = "Simplex.toml";

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    /// Standard I/O errors.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Errors when parsing TOML configuration files.
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Errors when parsing TOML configuration files.
    #[error("Unable to deserialize config: {0}")]
    UnableToDeserialize(toml::de::Error),

    /// Errors when parsing env variable.
    #[error("Unable to get env variable: {0}")]
    UnableToGetEnv(#[from] std::env::VarError),

    /// Errors when getting a path to config.
    #[error("Path doesn't a file: '{0}'")]
    PathIsNotFile(PathBuf),

    /// Errors when getting a path to config.
    #[error("Path doesn't exist: '{0}'")]
    PathNotExist(PathBuf),

    /// Config is missing.
    #[error("Config is missing in path: '{0}'")]
    MissingConfig(PathBuf),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub provider_config: ProviderConf,
    pub test_config: ElementsDConf,
    pub build_config: Option<BuildConf>,
}

#[derive(Debug, Clone)]
pub struct ProviderConf {
    simplicity_network: SimplicityNetwork,
}

#[derive(Debug, Default, Clone)]
pub struct ConfigOverride {
    pub rpc_creds: Option<ElementsDConf>,
    pub network: Option<SimplicityNetwork>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildConf {
    pub compile_simf: Vec<PathBuf>,
    pub out_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct _BuildConf {
    compile_simf: Vec<String>,
    out_dir: PathBuf,
}

impl Default for ProviderConf {
    fn default() -> Self {
        ProviderConf {
            simplicity_network: SimplicityNetwork::LiquidTestnet,
        }
    }
}

impl Config {
    pub fn get_path() -> Result<PathBuf, ConfigError> {
        let cwd = std::env::current_dir()?;
        Ok(cwd.join(CONFIG_FILENAME))
    }

    pub fn discover(cfg_override: Option<&ConfigOverride>) -> Result<Config, ConfigError> {
        match Config::_discover() {
            Ok(mut cfg) => {
                if let Some(cfg_override) = cfg_override {
                    if let Some(test_conf) = cfg_override.rpc_creds.clone() {
                        cfg.test_config = test_conf;
                    }
                    if let Some(network) = cfg_override.network {
                        cfg.provider_config.simplicity_network = network;
                    }
                }
                Ok(cfg)
            }
            Err(e) => Err(e),
        }
    }

    pub fn load(path_buf: impl AsRef<Path>) -> Result<Self, ConfigError> {
        Self::from_path(&path_buf)
    }

    pub fn load_or_discover(path_buf: Option<impl AsRef<Path>>) -> Result<Self, ConfigError> {
        match path_buf {
            Some(path) => Self::load(path),
            None => Self::_discover(),
        }
    }

    fn _discover() -> Result<Config, ConfigError> {
        let path = Self::get_path()?;
        dbg!(&path);
        if !path.is_file() {
            return Err(ConfigError::PathIsNotFile(path));
        }
        if !path.exists() {
            return Err(ConfigError::PathNotExist(path));
        }
        Ok(Config::from_path(&path)?)
    }

    fn from_path(p: impl AsRef<Path>) -> Result<Self, ConfigError> {
        std::fs::read_to_string(p.as_ref())?.parse()
    }
}

impl FromStr for Config {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cfg: _Config = toml::from_str(s).map_err(ConfigError::UnableToDeserialize)?;
        Ok(Config {
            provider_config: ProviderConf {
                simplicity_network: cfg.network.unwrap_or_default().into(),
            },
            test_config: cfg
                .test
                .map(|x| ElementsDConf {
                    elemendsd_path: x
                        .elementsd_path
                        .unwrap_or(ElementsDConf::obtain_default_elementsd_path()),
                    rpc_creds: x.rpc_creds.unwrap_or_default(),
                })
                .unwrap_or(ElementsDConf {
                    elemendsd_path: ElementsDConf::obtain_default_elementsd_path(),
                    rpc_creds: RpcCreds::None,
                }),
            build_config: match cfg.build {
                None => None,
                Some(x) => Some(BuildConf {
                    compile_simf: resolve_glob_paths(&x.compile_simf)?,
                    out_dir: resolve_dir_path(x.out_dir)?,
                }),
            },
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct _Config {
    network: Option<_NetworkName>,
    test: Option<TestingConfig>,
    build: Option<_BuildConf>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TestingConfig {
    elementsd_path: Option<PathBuf>,
    rpc_creds: Option<RpcCreds>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum _NetworkName {
    #[default]
    Liquid,
    LiquidTestnet,
    ElementsRegtest,
}

impl Into<SimplicityNetwork> for _NetworkName {
    fn into(self) -> SimplicityNetwork {
        match self {
            _NetworkName::Liquid => SimplicityNetwork::Liquid,
            _NetworkName::LiquidTestnet => SimplicityNetwork::LiquidTestnet,
            _NetworkName::ElementsRegtest => SimplicityNetwork::default_regtest(),
        }
    }
}

fn resolve_glob_paths(pattern: &[impl AsRef<str>]) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for path in pattern.iter().map(|x| resolve_glob_path(x.as_ref())) {
        let path = path?;
        paths.extend_from_slice(&path);
    }
    Ok(paths)
}

fn resolve_glob_path(pattern: impl AsRef<str>) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for path in glob::glob(pattern.as_ref())
        .map_err(|e| io::Error::other(e.to_string()))?
        .filter_map(Result::ok)
    {
        println!("path: '{}', pattern: '{}'", path.display(), pattern.as_ref());
        paths.push(path);
    }
    Ok(paths)
}

fn resolve_dir_path(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let mut path_outer = PathBuf::from(path.as_ref());

    if !path_outer.is_absolute() {
        let manifest_dir = std::env::current_dir()?;

        let mut path_local = PathBuf::from(manifest_dir);
        path_local.push(path_outer);

        path_outer = path_local;
    }

    if path_outer.extension().is_some() {
        return Err(io::Error::other(format!(
            "Folder can't have an extension, path: '{}'",
            path_outer.display()
        )));
    }
    if path_outer.is_file() {
        return Err(io::Error::other(format!(
            "Folder can't be a path, path: '{}'",
            path_outer.display()
        )));
    }
    Ok(path_outer)
}
