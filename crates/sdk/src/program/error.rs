/// Errors that can occur when compiling, preparing, and executing Simplicity programs.
#[derive(Debug, thiserror::Error)]
pub enum ProgramError {
    /// Error thrown when compiling the raw Simplicity program source fails.
    #[error("Failed to compile Simplicity program: {0}")]
    Compilation(String),

    /// Error indicating failure while matching or satisfying witness values to the program requirements.
    #[error("Failed to satisfy witness: {0}")]
    WitnessSatisfaction(String),

    /// Error indicating pruning the node tree during execution failed safely.
    #[error("Failed to prune program: {0}")]
    Pruning(#[from] simplicityhl::simplicity::bit_machine::ExecutionError),

    /// Error thrown when the bit machine cannot be initialized due to complexity or limit restrictions.
    #[error("Failed to construct a Bit Machine with enough space: {0}")]
    BitMachineCreation(#[from] simplicityhl::simplicity::bit_machine::LimitError),

    /// Error thrown during bit machine execution due to underlying logical or environment validation errors.
    #[error("Failed to execute program on the Bit Machine: {0}")]
    Execution(simplicityhl::simplicity::bit_machine::ExecutionError),

    /// Error indicating an input index points past the bounds of available inputs/UTXOs.
    #[error("UTXO index {input_index} out of bounds (have {utxo_count} UTXOs)")]
    UtxoIndexOutOfBounds {
        /// The requested input index to spend.
        input_index: usize,
        /// The actual total number of available UTXOs.
        utxo_count: usize,
    },

    /// Error indicating the script pubkey present on the targeted UTXO differs from the expectation.
    #[error("Script pubkey mismatch: expected hash {expected_hash}, got {actual_hash}")]
    ScriptPubkeyMismatch {
        /// The expected CMR (Commitment Merkle Root) hash.
        expected_hash: String,
        /// The actual CMR hash present in the script pubkey.
        actual_hash: String,
    },

    /// Error thrown when an underlying Elements transaction fails to extract from the PSET wrapper.
    #[error("Failed to extract tx from pst: {0}")]
    TxExtraction(#[from] simplicityhl::elements::pset::Error),

    /// Error indicating an array size overflow if the target index exceeds limits.
    #[error("Input index exceeds u32 maximum: {0}")]
    InputIndexOverflow(#[from] std::num::TryFromIntError),

    /// Error thrown if the compiled program fails to export or generate valid ABI metadata.
    #[error("Failed to obtain program witness types: {0}")]
    ProgramGenAbiMeta(String),
}
