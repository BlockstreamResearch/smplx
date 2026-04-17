use crate::program::ProgramError;
use crate::provider::ProviderError;

#[derive(Debug, thiserror::Error)]
pub enum SignerError {
    #[error(transparent)]
    Program(#[from] ProgramError),

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error(transparent)]
    WtnsInjectError(#[from] WtnsWrappingError),

    #[error("Failed to parse a mnemonic: {0}")]
    Mnemonic(String),

    #[error("Failed to extract tx from pst: {0}")]
    TxExtraction(#[from] simplicityhl::elements::pset::Error),

    #[error("Failed to unblind txout: {0}")]
    Unblind(#[from] simplicityhl::elements::UnblindError),

    #[error("Failed to blind a PST: {0}")]
    PsetBlind(#[from] simplicityhl::elements::pset::PsetBlindError),

    #[error("Failed to construct a message for the input spending: {0}")]
    SighashConstruction(#[from] elements_miniscript::psbt::SighashError),

    #[error("Fee amount is too low: {0}")]
    DustAmount(i64),

    #[error("Not enough fee amount {0} to cover transaction costs: {1}")]
    NotEnoughFeeAmount(i64, u64),

    #[error("Not enough funds on account to cover transaction costs: {0}")]
    NotEnoughFunds(u64),

    #[error("Invalid secret key")]
    InvalidSecretKey(#[from] simplicityhl::elements::secp256k1_zkp::UpstreamError),

    #[error("Failed to derive a private key: {0}")]
    PrivateKeyDerivation(#[from] elements_miniscript::bitcoin::bip32::Error),

    #[error("Failed to construct a derivation path: {0}")]
    DerivationPath(String),

    #[error("Failed to construct a wpkh descriptor: {0}")]
    WpkhDescriptor(String),

    #[error("Failed to construct a slip77 descriptor: {0}")]
    Slip77Descriptor(String),

    #[error("Failed to convert a descriptor: {0}")]
    DescriptorConversion(#[from] elements_miniscript::descriptor::ConversionError),

    #[error("Failed to construct a wpkh address: {0}")]
    WpkhAddressConstruction(#[from] elements_miniscript::Error),

    #[error("Missing such witness field: {0}")]
    WtnsFieldNotFound(String),
}

#[derive(Debug, thiserror::Error)]
pub enum WtnsWrappingError {
    #[error("Failed to parse path")]
    ParsingError,

    #[error("Unsupported path type: {0}")]
    UnsupportedPathType(String),

    #[error("Path index out of bounds: len is {0}, got {1}")]
    IdxOutOfBounds(usize, usize),

    #[error("Root type mismatch: expected {0}, got {1}")]
    RootTypeMismatch(String, String),

    #[error("Path reached undefined branch of Either")]
    EitherBranchMismatch,
}
