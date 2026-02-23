use simplicityhl::elements::secp256k1_zkp;

#[derive(Debug, thiserror::Error)]
pub enum SimplexError {
    #[error("Fee amount is too low: {0}")]
    PstFailure(#[from] simplicityhl::elements::pset::Error),

    #[error("Failed to produce a signature: {0}")]
    SigningFailed(String),

    #[error("Fee amount is too low: {0}")]
    DustAmount(u64),

    #[error("Not enough fee amount {0} to cover transaction costs: {1}")]
    NotEnoughFeeAmount(u64, u64),

    #[error("Failed to compile Simplicity program: {0}")]
    Compilation(String),

    #[error("Failed to satisfy witness: {0}")]
    WitnessSatisfaction(String),

    #[error("Failed to prune program: {0}")]
    Pruning(#[from] simplicityhl::simplicity::bit_machine::ExecutionError),

    #[error("Failed to construct a Bit Machine with enough space: {0}")]
    BitMachineCreation(#[from] simplicityhl::simplicity::bit_machine::LimitError),

    #[error("Failed to execute program on the Bit Machine: {0}")]
    Execution(simplicityhl::simplicity::bit_machine::ExecutionError),

    #[error("UTXO index {input_index} out of bounds (have {utxo_count} UTXOs)")]
    UtxoIndexOutOfBounds { input_index: usize, utxo_count: usize },

    #[error("Script pubkey mismatch: expected hash {expected_hash}, got {actual_hash}")]
    ScriptPubkeyMismatch { expected_hash: String, actual_hash: String },

    #[error("Input index exceeds u32 maximum: {0}")]
    InputIndexOverflow(#[from] std::num::TryFromIntError),

    #[error("Invalid seed length: expected 32 bytes, got {0}")]
    InvalidSeedLength(usize),

    #[error("Invalid secret key")]
    InvalidSecretKey(#[from] secp256k1_zkp::UpstreamError),

    #[error("HTTP request failed: {0}")]
    Request(String),

    #[error("Broadcast failed with HTTP {status} for {url}: {message}")]
    BroadcastRejected { status: u16, url: String, message: String },

    #[error("Failed to deserialize response: {0}")]
    Deserialize(String),

    #[error("Invalid txid format: {0}")]
    InvalidTxid(String),
}
