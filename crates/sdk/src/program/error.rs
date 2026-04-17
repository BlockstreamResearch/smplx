#[derive(Debug, thiserror::Error)]
pub enum ProgramError {
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

    #[error("Failed to extract tx from pst: {0}")]
    TxExtraction(#[from] simplicityhl::elements::pset::Error),

    #[error("Input index exceeds u32 maximum: {0}")]
    InputIndexOverflow(#[from] std::num::TryFromIntError),

    #[error("Failed to obtain program witness types: {0}")]
    ProgramGenAbiMeta(String),
}
