use simplex_runtime::ExplorerError;

#[derive(thiserror::Error, Debug)]
pub enum TestError {
    #[error("Explorer error occurred: {0}")]
    Explorer(#[from] ExplorerError),

    #[error("Unhealthy rpc connection, error: {0}")]
    UnhealthyRpc(ExplorerError),

    #[error("Node failed to start, error: {0}")]
    NodeFailedToStart(String),
}
