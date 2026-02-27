use crate::TestError;
use electrsd::bitcoind::bitcoincore_rpc::jsonrpc::serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const TEST_ENV_NAME: &str = "SIMPLEX_TEST_ENV";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementsDConf {
    pub elemendsd_path: PathBuf,
    pub rpc_creds: RpcCreds,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum RpcCreds {
    Auth {
        url: String,
        username: String,
        password: String,
    },
    #[default]
    None,
}

impl ElementsDConf {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, TestError> {
        let mut content = String::new();
        let mut file = OpenOptions::new().read(true).open(path)?;
        file.read_to_string(&mut content)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn obtain_default_elementsd_path() -> PathBuf {
        // TODO: change binary into installed one in $PATH dir
        const ELEMENTSD_BIN_PATH: &str = "../../assets/elementsd";
        const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

        Path::new(MANIFEST_DIR).join(ELEMENTSD_BIN_PATH)
    }
}
