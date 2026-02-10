use simplicityhl::simplicity::hex::HexToArrayError;

/// Errors that can occur when using the Simplex CLI.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Errors related to configuration loading or validation.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Standard I/O errors.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Errors when parsing TOML configuration files.
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Errors related to Partially Signed Elements Transactions (PSET).
    #[error("PSET error: {0}")]
    Pset(#[from] simplicityhl::elements::pset::Error),

    /// Errors when converting hex strings to byte arrays.
    #[error("Hex to array error: {0}")]
    HexToArray(#[from] HexToArrayError),
}
