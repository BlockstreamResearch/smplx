#[derive(thiserror::Error, Debug)]
pub enum RpcError {
    #[error(transparent)]
    ElementsRpcError(#[from] electrsd::bitcoind::bitcoincore_rpc::Error),

    #[error("Elements RPC returned an unexpected value for call {0}")]
    ElementsRpcUnexpectedReturn(String),

    #[error("Failed to decode hex value to array, {0}")]
    BitcoinHashesHex(#[from] bitcoin_hashes::hex::HexToArrayError),
}
