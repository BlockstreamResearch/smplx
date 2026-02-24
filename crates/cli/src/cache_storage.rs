use crate::error::Error;
use simplex_test::TestConfig;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct CacheStorage {}

impl CacheStorage {
    pub fn save_cached_test_config(test_config: &TestConfig) -> Result<PathBuf, Error> {
        let cache_dir = Self::get_cache_dir()?;
        std::fs::create_dir_all(&cache_dir)?;
        let test_config_cache_name = Self::create_test_cache_name(&cache_dir);

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&test_config_cache_name)?;
        file.write(toml::to_string_pretty(&test_config).unwrap().as_bytes())?;
        file.flush()?;
        Ok(test_config_cache_name)
    }

    pub fn get_cache_dir() -> Result<PathBuf, Error> {
        const TARGET_DIR_NAME: &str = "target";
        const SIMPLEX_CACHE_DIR_NAME: &str = "simplex";

        let cwd = std::env::current_dir()?;
        Ok(cwd.join(TARGET_DIR_NAME).join(SIMPLEX_CACHE_DIR_NAME))
    }

    pub fn create_test_cache_name(path: impl AsRef<Path>) -> PathBuf {
        const TEST_CACHE_NAME: &str = "test_config.toml";

        path.as_ref().join(TEST_CACHE_NAME)
    }
}
