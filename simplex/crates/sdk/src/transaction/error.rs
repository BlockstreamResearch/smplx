#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("Invalid signature type requested: {0}")]
    SignatureRequest(String),
}
