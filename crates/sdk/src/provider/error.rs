use crate::provider::rpc::error::RpcError;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error(transparent)]
    Rpc(#[from] RpcError),

    #[error("HTTP request failed: {0}")]
    Request(String),

    #[error("Couldn't wait for the transaction to be confirmed")]
    Confirmation(),

    #[error("Broadcast failed with HTTP {status} for {url}: {message}")]
    BroadcastRejected { status: u16, url: String, message: String },

    #[error("Failed to deserialize response: {0}")]
    Deserialize(String),

    #[error("Invalid txid format: {0}")]
    InvalidTxid(String),
}
