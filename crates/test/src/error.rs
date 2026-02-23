use electrsd::electrum_client::bitcoin::hex::HexToArrayError;
use simplex_provider::ExplorerError;

#[derive(thiserror::Error, Debug)]
pub enum TestError {
    #[error("Explorer error occurred: {0}")]
    Explorer(#[from] ExplorerError),

    #[error("Unhealthy rpc connection, error: {0}")]
    UnhealthyRpc(ExplorerError),

    #[error("Node failed to start, error: {0}")]
    NodeFailedToStart(String),

    /// Errors when converting hex strings to byte arrays.
    #[error("Hex to array error: '{0}'")]
    HexToArray(#[from] HexToArrayError),

    /// Errors when failed to decode transaction.
    #[error("Failed to decode transaction: '{0}'")]
    TransactionDecode(String),
}
