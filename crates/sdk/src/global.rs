use std::sync::OnceLock;

use crate::program::TrackerLogLevel;

/// A structure to represent the global configuration settings for the application.
#[derive(Clone, Copy, Debug)]
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

/// Sets the global configuration for the SDK.
///
/// This function allows setting a global configuration which includes
/// the logging level for `simplicity` contracts execution.
/// It must be called exactly once during the application's lifetime.
///
/// # Errors
/// Returns an error containing the newly created `GlobalConfig` if the global configuration has already been initialized.
pub fn set_global_config(log_level: TrackerLogLevel) -> Result<(), GlobalConfig> {
    GLOBAL_CONFIG.set(GlobalConfig { log_level })
}

/// Returns the default log level if `GLOBAL_CONFIG` is not initialized
pub fn get_log_level() -> TrackerLogLevel {
    GLOBAL_CONFIG
        .get()
        .map_or(GlobalConfig::default().log_level, |config| config.log_level)
}
