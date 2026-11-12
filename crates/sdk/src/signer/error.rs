use elements_miniscript::bitcoin::bip32::DerivationPath;

use crate::program::ProgramError;
use crate::provider::ProviderError;
use crate::transaction::error::ProgramInputError;
use crate::transaction::{FinalTransactionError, PartialInputError, UtxoError, WitnessWrappingError};

/// Errors from a [`Signer`](super::Signer).
///
/// A contract that rejects a spend fails with `Program(ProgramError::ProgramExecution { .. })`
/// before anything is broadcast.
/// A transaction that is broadcast and refused fails with
/// `Provider(ProviderError::BroadcastRejected { .. })`.
#[derive(Debug, thiserror::Error)]
pub enum SignerError {
    /// A program failed.
    ///
    /// - `ProgramError::ProgramExecution`: a program input failed to run while the transaction was
    ///   measured or finalized.
    /// - Any other variant: the program doesn't compile, or the input doesn't spend its address.
    ///   The signer finds this while signing for a
    ///   [`ProgramSignature`](crate::transaction::ProgramSignature), before the program runs.
    #[error(transparent)]
    Program(#[from] ProgramError),

    /// Errors originating from provider network interactions.
    #[error(transparent)]
    Provider(#[from] ProviderError),

    /// A UTXO is missing its asset or amount and has no unblinding secrets.
    #[error(transparent)]
    Utxo(#[from] UtxoError),

    /// An input's asset and amount are unknown, or its unblinding secrets don't open the output it
    /// spends.
    #[error(transparent)]
    PartialInput(#[from] PartialInputError),

    /// Errors originating from transaction construction or validation.
    #[error(transparent)]
    FinalTransaction(#[from] FinalTransactionError),

    /// Error indicating a provider-backed operation was requested on a signer built without one.
    #[cfg(feature = "provider")]
    #[error("This signer was constructed without a provider")]
    ProviderUnavailable,

    /// Errors encountered when attempting to inject or wrap witness fields.
    #[error(transparent)]
    WitnessInjectError(#[from] WitnessWrappingError),

    /// Error indicating an incorrectly formatted mnemonic phrase.
    #[error("Failed to parse a mnemonic: {0}")]
    Mnemonic(String),

    /// Error thrown when PSET transaction extraction fails.
    #[error("Failed to extract tx from pst: {0}")]
    TxExtraction(#[from] simplicityhl::elements::pset::Error),

    // Boxed because `UnblindError` would push every `SignerError` over clippy's 128-byte
    // `result_large_err` limit. Since SimplicityHL 0.8.0 (elements 0.27.0), `UnblindError` has a
    // `RangeProofMessage(RangeProofMessageError)` variant whose `ConfidentialAssetMismatch` arm
    // holds two 64-byte `Generator`s (`in_txout`, `in_message`), making it 132 bytes.
    /// Error indicating failure to unblind a confidential transaction output.
    #[error("Failed to unblind txout: {0}")]
    Unblind(#[source] Box<simplicityhl::elements::UnblindError>),

    /// Error thrown when PSET blinding fails.
    #[error("Failed to blind a PST: {0}")]
    PsetBlind(#[from] simplicityhl::elements::pset::PsetBlindError),

    /// Error indicating failure to construct sighash for input spending.
    #[error("Failed to construct a message for the input spending: {0}")]
    SighashConstruction(#[from] elements_miniscript::psbt::SighashError),

    /// Error indicating the transaction is not balanced.
    #[error("Transaction is not balanced or malformed")]
    Unbalanced,

    /// Error indicating the transaction inputs cover an amount that is lower than the dust limit.
    #[error("Fee amount is too low: {0}")]
    DustAmount(i64),

    /// Error indicating the defined fee amount cannot cover the calculated transaction costs.
    #[error("Not enough fee amount {0} to cover transaction costs: {1}")]
    NotEnoughFeeAmount(i64, u64),

    /// Error indicating a fee rate that is not a finite, non-negative number of sats/kvb.
    #[error("Invalid fee rate: {0} sats/kvb")]
    InvalidFeeRate(f32),

    /// Error indicating that the available UTXO funds are not enough to cover total costs.
    #[error("Not enough funds on account to cover transaction costs: {0}")]
    NotEnoughFunds(u64),

    /// Error indicating a confidential input was left with no blinded output to balance against.
    ///
    /// Raised when a transaction spending confidential inputs has an explicit change target, or is
    /// finalized without change, which the node would reject as `bad-txns-in-ne-out`.
    #[error("A confidential input needs at least one blinded output to balance against")]
    ConfidentialInputWithoutBlindedOutput,

    /// A derivation path doesn't have exactly two unhardened steps, such as `0/5`.
    #[error("The derivation path `{0}` must have exactly two unhardened steps, such as 0/5")]
    InvalidDerivationPath(DerivationPath),

    /// A native input spends a P2WPKH output locked to a different key than the one at its
    /// derivation path.
    #[error("Input {0} is locked to a different key than the one at its derivation path")]
    KeyMismatch(usize),

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
}

impl From<ProgramInputError> for SignerError {
    fn from(err: ProgramInputError) -> Self {
        match err {
            ProgramInputError::Program(err) => Self::Program(err),
            ProgramInputError::Injection(err) => Self::WitnessInjectError(err),
        }
    }
}

impl From<simplicityhl::elements::UnblindError> for SignerError {
    fn from(err: simplicityhl::elements::UnblindError) -> Self {
        Self::Unblind(Box::new(err))
    }
}
