use crate::provider::rpc::error::RpcError;

/// Defines standard errors possible when using a blockchain interaction provider.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// Wrapper around an RPC-level error representing transport or network-level connectivity failures to the inner node.
    #[error(transparent)]
    Rpc(#[from] RpcError),

    /// Error indicating that a standard HTTP request to a provider (such as an Esplora REST instance) encountered a failure.
    #[error("HTTP request failed: {0}")]
    Request(String),

    /// Error indicating the configured timeout to wait for transaction confirmation elapsed without confirmation.
    #[error("Couldn't wait for the transaction to be confirmed")]
    Confirmation(),

    /// Error indicating a provider returned a non-success response explicitly rejecting a broadcasted transaction payload.
    #[error("Broadcast failed with HTTP {status} for {url}: {message}")]
    BroadcastRejected {
        /// The HTTP status code indicating the failure.
        status: u16,
        /// The URL that the broadcast was sent to.
        url: String,
        /// The error message returned by the provider.
        message: String,
    },

    /// Error indicating that a provider's raw response body was unable to be correctly deserialized into native structs or types.
    #[error("Failed to deserialize response: {0}")]
    Deserialize(String),

    /// Error indicating an incorrectly formatted transaction ID string was encountered.
    #[error("Invalid txid format: {0}")]
    InvalidTxid(String),
}
