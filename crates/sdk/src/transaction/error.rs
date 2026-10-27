use simplicityhl::elements::OutPoint;

/// Errors encountered while assembling or validating a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
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
