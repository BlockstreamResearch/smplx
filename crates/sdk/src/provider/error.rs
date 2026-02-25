#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP request failed: {0}")]
    Request(String),

    #[error("Broadcast failed with HTTP {status} for {url}: {message}")]
    BroadcastRejected { status: u16, url: String, message: String },

    #[error("Failed to deserialize response: {0}")]
    Deserialize(String),

    #[error("Invalid txid format: {0}")]
    InvalidTxid(String),
}
