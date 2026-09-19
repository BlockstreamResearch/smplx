use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use super::error::RegtestError;

pub const DEFAULT_REGTEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";
pub const DEFAULT_BITCOINS: u64 = 10_000_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RegtestConfig {
    pub mnemonic: String,
    pub bitcoins: u64,
    pub rpc_port: Option<u16>,
    pub esplora_port: Option<u16>,
    pub rpc_user: Option<String>,
    pub rpc_password: Option<String>,
}

impl RegtestConfig {
    /// Loads a `RegtestConfig` from a specified TOML file.
    ///
    /// # Errors
    /// Returns a `RegtestError` if the file cannot be opened, read, or if the contents are not valid TOML.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, RegtestError> {
        let mut content = String::new();
        let mut file = OpenOptions::new().read(true).open(path)?;

        file.read_to_string(&mut content)?;

        Ok(toml::from_str(&content)?)
    }
}

impl Default for RegtestConfig {
    fn default() -> Self {
        Self {
            mnemonic: DEFAULT_REGTEST_MNEMONIC.to_string(),
            bitcoins: DEFAULT_BITCOINS,
            rpc_port: None,
            esplora_port: None,
            rpc_user: None,
            rpc_password: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_config_is_loaded_and_defaults_are_safe() {
        let path = std::env::temp_dir().join(format!("smplx-regtest-config-{}.toml", std::process::id()));
        std::fs::write(
            &path,
            r#"
                mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
                bitcoins = 42
                rpc_port = 18443
                esplora_port = 3000
                rpc_user = "user"
                rpc_password = "password"
            "#,
        )
        .expect("regtest config should be writable");

        let loaded = RegtestConfig::from_file(&path).expect("regtest config should load");
        let defaults = RegtestConfig::default();

        assert_eq!(loaded.bitcoins, 42);
        assert_eq!(loaded.rpc_port, Some(18443));
        assert_eq!(loaded.esplora_port, Some(3000));
        assert_eq!(loaded.rpc_user.as_deref(), Some("user"));
        assert_eq!(loaded.rpc_password.as_deref(), Some("password"));
        assert!(defaults.rpc_port.is_none());
        assert!(defaults.esplora_port.is_none());

        let _ = std::fs::remove_file(path);
    }
}
