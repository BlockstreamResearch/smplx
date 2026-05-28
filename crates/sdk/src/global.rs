use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::program::TrackerLogLevel;

static GLOBAL_CONFIG: OnceLock<GlobalConfig> = OnceLock::new();

/// A structure to represent the global configuration settings for the application.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlobalConfig {
    verbosity: Verbosity,
}

impl GlobalConfig {
    /// Sets the global configuration for the SDK.
    ///
    /// This function allows setting a global configuration which includes
    /// the logging level for `simplicity` contracts execution.
    /// It must be called exactly once during the application's lifetime.
    ///
    /// # Errors
    /// Returns an error containing the newly created `GlobalConfig` if the global configuration has already been initialized.
    pub fn set_global_config(verbosity: Verbosity) -> Result<(), GlobalConfig> {
        GLOBAL_CONFIG.set(GlobalConfig { verbosity })
    }

    /// Returns the default log level if `GLOBAL_CONFIG` is not initialized
    pub fn get_log_level() -> TrackerLogLevel {
        GLOBAL_CONFIG
            .get()
            .map_or(GlobalConfig::default().verbosity.tracker_log_level(), |config| {
                config.verbosity.tracker_log_level()
            })
    }

    /// Returns whether the global configuration includes debug symbols,
    /// defaulting to `false` if `GLOBAL_CONFIG` is not initialized.
    pub fn get_include_debug_symbols() -> bool {
        GLOBAL_CONFIG
            .get()
            .map_or(GlobalConfig::default().verbosity.include_debug_symbols(), |config| {
                config.verbosity.include_debug_symbols()
            })
    }

    /// Returns `true` if the log level corresponds to [`Verbosity::MAX_VERBOSITY_LEVEL`]
    ///
    /// Equivalent to the `-vv` flag passed to `simplex test`.
    #[must_use]
    pub fn is_max_verbose() -> bool {
        Self::get_log_level() == Verbosity::new(Verbosity::MAX_VERBOSITY_LEVEL).tracker_log_level()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
/// An enumeration to represent the verbosity levels of the Simplicity execution logging.
pub enum Verbosity {
    /// No logs will be printed.
    #[default]
    None = 0,
    /// The debug mode
    Debug = 1,
    /// The trace mode
    Trace = 2,
}

impl Verbosity {
    /// The maximum allowed verbosity level.
    pub const MAX_VERBOSITY_LEVEL: u8 = 2;

    /// Creates a `Verbosity` instance from the number of verbosity flags provided (e.g., -v, -vv).
    #[must_use]
    pub fn new(flags: u8) -> Self {
        match flags {
            0 => Verbosity::None,
            1 => Verbosity::Debug,
            _ => Verbosity::Trace,
        }
    }

    /// Converts the `Verbosity` level to a corresponding `TrackerLogLevel`.
    #[must_use]
    pub fn tracker_log_level(&self) -> TrackerLogLevel {
        match self {
            Verbosity::None => TrackerLogLevel::None,
            Verbosity::Debug => TrackerLogLevel::Debug,
            Verbosity::Trace => TrackerLogLevel::Trace,
        }
    }

    /// Determines if the current verbosity level includes debug symbols.
    #[must_use]
    pub fn include_debug_symbols(&self) -> bool {
        matches!(self, Verbosity::Debug | Verbosity::Trace)
    }
}
