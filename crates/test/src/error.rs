use std::io;

use smplx_sdk::provider::ProviderError;

use smplx_regtest::error::RegtestError;

#[derive(thiserror::Error, Debug)]
pub enum TestError {
    #[error(transparent)]
    Regtest(#[from] RegtestError),

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error("Failed to deserialize config: '{0}'")]
    ConfigDeserialize(#[from] toml::de::Error),

    #[error("io error occurred: '{0}'")]
    Io(#[from] io::Error),

    #[error("Network name should either be `Liquid`, `LiquidTestnet` or `ElementsRegtest`, got: {0}")]
    BadNetworkName(String),

    #[error("Occurred a network utils execution error: '{0}'")]
    NetworkUtilsExecution(#[from] NetworkUtilsError),
}

#[derive(thiserror::Error, Debug)]
pub enum NetworkUtilsError {
    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error("Unsuccessful action completion, err: '{0}'")]
    UnsuccessfulSync(String),

    #[error(transparent)]
    TimeShift(#[from] TimeShiftError),
}

#[derive(thiserror::Error, Debug)]
pub enum TimeShiftError {
    #[error("Invalid mock time '{timestamp}': expected 0 or a timestamp at least '{minimum}'")]
    InvalidMockTime { timestamp: u64, minimum: u64 },

    #[error(
        "Mock time can only move forward: timestamp '{timestamp}' must be greater than current MTP '{current_mtp}'"
    )]
    NotForward { current_mtp: u64, timestamp: u64 },

    #[error("Node returned a negative median-time-past: '{current_mtp}'")]
    InvalidMedianTime { current_mtp: i64 },

    #[error("Cannot advance MTP because block height '{current_height}' would overflow")]
    BlockHeightOverflow { current_height: u64 },

    #[error("Failed to advance MTP to '{expected}', node reported '{actual}'")]
    UnexpectedMedianTime { expected: u64, actual: u64 },

    #[error("Failed to get the system time for verification: '{0}'")]
    SystemTimeError(#[from] std::time::SystemTimeError),
}
