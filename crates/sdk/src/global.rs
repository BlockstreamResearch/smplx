use std::sync::OnceLock;

use crate::program::TrackerLogLevel;

#[derive(Clone, Copy)]
pub struct GlobalConfig {
    log_level: TrackerLogLevel,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            log_level: TrackerLogLevel::Debug,
        }
    }
}

static GLOBAL_CONFIG: OnceLock<GlobalConfig> = OnceLock::new();

pub fn set_global_config(log_level: TrackerLogLevel) -> Result<(), GlobalConfig> {
    GLOBAL_CONFIG.set(GlobalConfig { log_level })
}

/// Returns default log level if `GLOBAL_CONFIG` is not initialized
pub fn get_log_level() -> TrackerLogLevel {
    GLOBAL_CONFIG
        .get()
        .map_or(GlobalConfig::default().log_level, |config| config.log_level)
}
