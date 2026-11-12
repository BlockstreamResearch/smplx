use simplicityhl::elements::OutPoint;
use simplicityhl::elements::pset::PsetBlindError;

use crate::program::ProgramError;

/// Errors encountered while assembling, validating or blinding a transaction.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FinalTransactionError {
    /// An input has no known asset or amount and no unblinding secrets.
    #[error("Input {input_index} requires an explicit asset and amount or unblinding secrets")]
    MissingInputSecrets {
        /// The index of the unresolved input in the transaction.
        input_index: usize,
    },
    /// An issuance is invalid.
    #[error("Input {input_index} doesn't issue anything")]
    EmptyIssuance {
        /// The index of the issuance input in the transaction.
        input_index: usize,
    },
    /// A reissuance spends a token without a non-zero asset blinding factor.
    #[error("Input {input_index} reissues with a token that isn't blinded")]
    UnblindedReissuanceToken {
        /// The index of the reissuance input in the transaction.
        input_index: usize,
    },
    /// The difference of the policy asset amounts of the inputs and outputs overflows `i64`.
    #[error("The policy asset amounts overflow the fee delta")]
    AmountOverflow,
    /// The transaction spends the same outpoint more than once.
    #[error("The transaction spends {0} more than once")]
    DuplicateInput(OutPoint),
    /// An output without a blinding key chooses a blinding factor.
    #[error("Output {output_index} chooses a blinding factor but has no blinding key")]
    ChosenFactorOnExplicitOutput {
        /// The index of the output in the transaction.
        output_index: usize,
    },
    /// An output chooses the asset blinding factor an input brings the same asset in with. Its
    /// asset commitment then equals the input's, and no surjection proof can be made.
    #[error("Output {output_index} chooses the asset blinding factor of an input with the same asset")]
    SurjectionWithoutShift {
        /// The index of the output in the transaction.
        output_index: usize,
    },
    /// Every blinded output chooses its value blinding factor, so none is left to balance the
    /// others.
    #[error("Every blinded output chooses its value blinding factor, so none can balance the transaction")]
    NoFreeValueFactor,
    /// A commitment or a proof of a blinded output can't be made.
    #[error("Failed to blind the transaction: {0}")]
    Blinding(#[from] PsetBlindError),
}

/// Errors encountered when reading a UTXO's asset or amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UtxoError {
    /// Neither an explicit asset ID nor unblinding secrets are available.
    #[error("UTXO {0} has no explicit asset or unblinding secrets")]
    UnknownAsset(OutPoint),
    /// Neither an explicit amount nor unblinding secrets are available.
    #[error("UTXO {0} has no explicit amount or unblinding secrets")]
    UnknownAmount(OutPoint),
}

/// Errors encountered when checking the asset and amount an input spends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PartialInputError {
    /// The spent UTXO is confidential and the input has no unblinding secrets.
    #[error("The input spending {0} is confidential and has no unblinding secrets, so its amount is unknown")]
    UnknownAmount(OutPoint),
    /// The unblinding secrets don't rebuild the spent UTXO's asset.
    #[error("The unblinding secrets of the input spending {0} don't open its asset")]
    AssetMismatch(OutPoint),
    /// The unblinding secrets rebuild the spent UTXO's asset but not its amount.
    #[error("The unblinding secrets of the input spending {0} don't open its amount")]
    AmountMismatch(OutPoint),
}

/// Errors from putting a signature into a program input's witness.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ProgramInputError {
    /// The program doesn't compile, so its witness types can't be read.
    #[error(transparent)]
    Program(#[from] ProgramError),

    /// The signature doesn't fit the witness.
    #[error(transparent)]
    Injection(#[from] WitnessWrappingError),
}

/// Errors originating from manipulating witness paths and injecting values.
#[derive(Debug, thiserror::Error)]
pub enum WitnessWrappingError {
    /// A path step isn't `Left`, `Right` or a decimal index.
    #[error("Failed to parse path")]
    ParsingError,

    /// A path step doesn't fit the type it steps into: an index into a type that isn't an array or
    /// tuple, or `Left` or `Right` into a type that isn't an `Either`.
    #[error("Unsupported path type: {0}")]
    UnsupportedPathType(String),

    /// An index is past the end of an array or tuple: `(length, index)`.
    #[error("Path index out of bounds: len is {0}, got {1}")]
    IdxOutOfBounds(usize, usize),

    /// The type at the end of the path isn't the signature's type: `(expected, signature type)`.
    #[error("Root type mismatch: expected {0}, got {1}")]
    RootTypeMismatch(String, String),

    /// A [`ProgramSignature`](super::ProgramSignature) with a path names a field that the
    /// program's witness doesn't have.
    #[error("The program's witness has no field `{0}`")]
    WitnessFieldNotFound(String),

    /// The supplied witness value takes the other branch of an `Either` than the path does.
    #[error("The witness value takes the other branch of an Either than the path")]
    EitherBranchMismatch,
}
