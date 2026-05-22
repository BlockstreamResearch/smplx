use std::fmt::Write;
use std::{cell::RefCell, sync::OnceLock};

use serde::{Deserialize, Serialize};

use crate::program::TrackerLogLevel;

static GLOBAL_CONFIG: OnceLock<GlobalConfig> = OnceLock::new();

/// A structure to represent the global configuration settings for the application.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlobalConfig {
    verbosity: Verbosity,
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

/// Returns `true` if the log level corresponds to the `-vv` flag passed to `simplex test`.
///
/// Equivalent to [`TrackerLogLevel::Trace`] being set as the global log level.
#[must_use]
pub fn is_verbose() -> bool {
    get_log_level() >= TrackerLogLevel::Trace
}

/// Helper struct for gathering info from `node.bounds()`
pub struct CostInfo {
    /// truncated cmr
    pub cmr: [u8; 8],
    /// inner value of `Cost`
    pub cost: u32,
    /// program size in bytes
    pub program_size: usize,
    /// witness size in bytes
    pub witness_size: usize,
}

/// Global logger state for buffering program execution output.
///
/// Buffers are flushed to stderr only on successful transaction finalization,
/// discarding output from intermediate estimation passes.
pub struct GlobalLogger {
    /// Cost metrics from the most recent program execution.
    pub cost_info: Option<CostInfo>,
    /// Execution trace lines in insertion order.
    pub trace_buffer: Vec<String>,
}

thread_local! {
    static GLOBAL_LOGGER: RefCell<GlobalLogger> = const { RefCell::new(GlobalLogger { cost_info: None, trace_buffer: Vec::new() }) };

}

/// Buffers a cost log entry for the given program source.
///
/// Overwrites any previous entry for the same source, ensuring only
/// the most recent execution's cost is reported.
pub fn buffer_trace_log(tracker_logs: String) {
    GLOBAL_LOGGER.with(|logger| logger.borrow_mut().trace_buffer.push(tracker_logs));
}

/// Buffers a line of execution trace output.
pub fn buffer_cost_log(stats: CostInfo) {
    GLOBAL_LOGGER.with(|logger| {
        logger.borrow_mut().cost_info = Some(stats);
    });
}
/// Flushes all buffered cost and trace output to stderr, then clears buffers.
///
/// Call this on successful transaction finalization.
pub fn flush_logs() {
    GLOBAL_LOGGER.with(|logger| {
        let logger = logger.borrow();

        if let Some(cost_info) = &logger.cost_info {
            let cmr_hex: String = cost_info.cmr.iter().fold(String::new(), |mut output, b| {
                let _ = write!(output, "{b:02x}");
                output
            });

            eprintln!(
                "Program info: cmr=[{}] cost={}wu  prog={}b  witness={}b",
                cmr_hex, cost_info.cost, cost_info.program_size, cost_info.witness_size,
            );
        }

        if !logger.trace_buffer.is_empty() {
            eprintln!("── trace ──────────────────");
            for msg in &logger.trace_buffer {
                eprintln!("{msg}");
            }
        }
    });

    clear_logs();
}

/// Discards all buffered output without printing.
pub fn clear_logs() {
    GLOBAL_LOGGER.with(|logger| {
        logger.borrow_mut().cost_info = None;
        logger.borrow_mut().trace_buffer.clear();
    });
}
