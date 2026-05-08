use crate::program::ProgramError;
use crate::provider::ProviderError;

/// Core error types for the Signer component.
#[derive(Debug, thiserror::Error)]
pub enum SignerError {
    /// Errors originating from Simplicity program evaluation and state.
    #[error(transparent)]
    Program(#[from] ProgramError),

    /// Errors originating from provider network interactions.
    #[error(transparent)]
    Provider(#[from] ProviderError),

    /// Errors encountered when attempting to inject or wrap witness fields.
    #[error(transparent)]
    WtnsInjectError(#[from] WtnsWrappingError),

    /// Error indicating an incorrectly formatted mnemonic phrase.
    #[error("Failed to parse a mnemonic: {0}")]
    Mnemonic(String),

    /// Error thrown when PSET transaction extraction fails.
    #[error("Failed to extract tx from pst: {0}")]
    TxExtraction(#[from] simplicityhl::elements::pset::Error),

    /// Error indicating failure to unblind a confidential transaction output.
    #[error("Failed to unblind txout: {0}")]
    Unblind(#[from] simplicityhl::elements::UnblindError),

    /// Error thrown when PSET blinding fails.
    #[error("Failed to blind a PST: {0}")]
    PsetBlind(#[from] simplicityhl::elements::pset::PsetBlindError),

    /// Error indicating failure to construct sighash for input spending.
    #[error("Failed to construct a message for the input spending: {0}")]
    SighashConstruction(#[from] elements_miniscript::psbt::SighashError),

    /// Error indicating the transaction inputs cover an amount that is lower than the dust limit.
    #[error("Fee amount is too low: {0}")]
    DustAmount(i64),

    /// Error indicating the defined fee amount cannot cover the calculated transaction costs.
    #[error("Not enough fee amount {0} to cover transaction costs: {1}")]
    NotEnoughFeeAmount(i64, u64),

    /// Error indicating that the available UTXO funds are not enough to cover total costs.
    #[error("Not enough funds on account to cover transaction costs: {0}")]
    NotEnoughFunds(u64),

    /// Error indicating an invalid upstream `secp256k1` secret key.
    #[error("Invalid secret key")]
    InvalidSecretKey(#[from] simplicityhl::elements::secp256k1_zkp::UpstreamError),

    /// Error thrown when HD wallet private key derivation fails.
    #[error("Failed to derive a private key: {0}")]
    PrivateKeyDerivation(#[from] elements_miniscript::bitcoin::bip32::Error),

    /// Error thrown when constructing a derivation path string fails.
    #[error("Failed to construct a derivation path: {0}")]
    DerivationPath(String),

    /// Error indicating failure to construct a valid WPKH (Witness Public Key Hash) descriptor.
    #[error("Failed to construct a wpkh descriptor: {0}")]
    WpkhDescriptor(String),

    /// Error indicating failure to construct a valid SLIP77 blinding key descriptor.
    #[error("Failed to construct a slip77 descriptor: {0}")]
    Slip77Descriptor(String),

    /// Error thrown if there's a problem during descriptor conversion.
    #[error("Failed to convert a descriptor: {0}")]
    DescriptorConversion(#[from] elements_miniscript::descriptor::ConversionError),

    /// Error thrown when WPKH address creation fails.
    #[error("Failed to construct a wpkh address: {0}")]
    WpkhAddressConstruction(#[from] elements_miniscript::Error),

    /// Error indicating an expected witness field could not be found.
    #[error("Missing such witness field: {0}")]
    WtnsFieldNotFound(String),
}

/// Errors originating from manipulating witness paths and injecting values.
#[derive(Debug, thiserror::Error)]
pub enum WtnsWrappingError {
    /// Error indicating a failure while parsing the provided witness path string.
    #[error("Failed to parse path")]
    ParsingError,

    /// Error pointing to the use of a path type that is currently not supported.
    #[error("Unsupported path type: {0}")]
    UnsupportedPathType(String),

    /// Error thrown during path traversal when an index exceeds the inner array lengths.
    #[error("Path index out of bounds: len is {0}, got {1}")]
    IdxOutOfBounds(usize, usize),

    /// Error indicating that the runtime type at the path root expected one type but encountered another.
    #[error("Root type mismatch: expected {0}, got {1}")]
    RootTypeMismatch(String, String),

    /// Error indicating that a path traversal attempted to reach an undefined or mismatched Either branch.
    #[error("Path reached undefined branch of Either")]
    EitherBranchMismatch,
}
