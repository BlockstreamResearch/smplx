#[cfg(feature = "provider")]
use crate::provider::rpc::error::RpcError;

/// Errors from a [`ProviderTrait`](super::ProviderTrait) implementation.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// An [`ElementsRpc`](super::ElementsRpc) call failed.
    #[cfg(feature = "provider")]
    #[error(transparent)]
    Rpc(#[from] RpcError),

    /// An HTTP request failed or timed out, or a query answered with a status other than 200.
    #[error("HTTP request failed: {0}")]
    Request(String),

    /// `wait` stopped polling before the transaction confirmed.
    #[error("Couldn't wait for the transaction to be confirmed")]
    Confirmation,

    /// The provider answered a broadcast with an HTTP status outside 200 to 299.
    ///
    /// This is how a node's refusal arrives, for consensus or policy reasons such as
    /// `non-BIP68-final` for a relative timelock that hasn't passed, with the node's reason in
    /// `message`. A rate limit or a server error arrives the same way, so check `status`.
    #[error("Broadcast failed with HTTP {status} for {url}: {message}")]
    BroadcastRejected {
        /// The HTTP status code indicating the failure.
        status: u16,
        /// The URL that the broadcast was sent to.
        url: String,
        /// The error message returned by the provider.
        message: String,
    },

    /// Error indicating the provider answered with another transaction.
    #[error("Expected transaction {expected}, the provider returned {returned}")]
    UnexpectedTxid {
        /// The transaction asked for or broadcast.
        expected: simplicityhl::elements::Txid,
        /// The transaction the provider returned.
        returned: simplicityhl::elements::Txid,
    },

    /// Error indicating the provider listed an output that isn't locked by the queried script.
    #[error("The provider listed {0}, which the queried script doesn't lock")]
    UnexpectedScript(simplicityhl::elements::OutPoint),

    /// Error indicating that a provider's raw response body was unable to be correctly deserialized into native structs or types.
    #[error("Failed to deserialize response: {0}")]
    Deserialize(String),

    /// Error indicating an incorrectly formatted transaction ID string was encountered.
    #[error("Invalid txid format: {0}")]
    InvalidTxid(String),

    /// Error indicating that the rpc returned a malformed response.
    #[error("Couldn't parse the response")]
    BadResponse,
}

/// Network conversion error.
#[derive(thiserror::Error, Debug)]
pub enum NetworkConvertError {
    /// The provided network name is not supported.
    #[error("Network name should either be `Liquid`, `LiquidTestnet` or `ElementsRegtest`, got: {0}")]
    BadNetworkName(String),
}
