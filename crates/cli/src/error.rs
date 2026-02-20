use simplicityhl::simplicity::hex::HexToArrayError;

/// Errors that can occur when using the Simplex CLI.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Errors related to configuration loading or validation.
    #[error("Configuration error: '{0}'")]
    Config(String),

    /// Standard I/O errors.
    #[error("IO error: '{0}'")]
    Io(#[from] std::io::Error),

    /// Errors related to Partially Signed Elements Transactions (PSET).
    #[error("PSET error: '{0}'")]
    Pset(#[from] simplicityhl::elements::pset::Error),

    /// Errors when converting hex strings to byte arrays.
    #[error("Hex to array error: '{0}'")]
    HexToArray(#[from] HexToArrayError),

    /// Errors when using test suite to run elementsd node in regtest.
    #[error("Occurred error with test suite, error: '{0}'")]
    Test(#[from] Box<simplex_test::TestError>),

    /// Errors when building config.
    #[error("Occurred error with config building, error: '{0}'")]
    ConfigError(#[from] crate::config::ConfigError),

    /// Errors when building config.
    #[error("Failed to discover config, check existence or create new one with `simplex init`, error: '{0}'")]
    ConfigDiscoveryFailure(crate::config::ConfigError),
}
