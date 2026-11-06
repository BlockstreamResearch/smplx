use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::{LockTime, Sequence};

/// Errors that can occur when compiling, preparing, and executing Simplicity programs.
#[derive(Debug, thiserror::Error)]
pub enum ProgramError {
    /// Running the program of a transaction input failed, so the transaction wasn't measured or
    /// finalized.
    ///
    /// Raised before broadcast. `source` says why: `ProgramError::Pruning` when the contract
    /// rejects the spend, and other variants, such as `ProgramError::WitnessSatisfaction`,
    /// `ProgramError::ScriptPubkeyMismatch` or `ProgramError::InsufficientBudget`, when the
    /// program can't run on this input or its witness stack doesn't cover the execution cost. The
    /// locktime and sequence identify most timelock failures.
    #[error(
        "Program input {input_index} did not execute (transaction locktime {locktime}, input sequence {sequence}): {source}"
    )]
    ProgramExecution {
        /// The index of the input whose program failed.
        input_index: usize,
        /// The locktime the transaction being satisfied carries.
        locktime: u32,
        /// The sequence of the failing input.
        sequence: u32,
        /// The underlying program failure.
        source: Box<ProgramError>,
    },

    /// The witness budget, set by the serialized witness stack size, doesn't cover the program's execution cost.
    ///
    /// Consensus requires `cost <= 1000 * (serialized witness stack size + 50)` milli weight units.
    /// The 50 is Elements' `VALIDATION_WEIGHT_OFFSET`
    /// ([script.h L61](https://github.com/ElementsProject/elements/blob/1af7a4d9bea93b4d7f29a77f9751a0e6e03a4390/src/script/script.h#L61)).
    /// The 1000 converts the weight-unit budget to libSimplicity's milli-weight-unit cost
    /// ([eval.c L790](https://github.com/BlockstreamResearch/simplicity/blob/91d89bac54e9411281ee3b6e766a0ea20eb0a3d3/C/eval.c#L790)).
    #[error(
        "Program cost {cost_wu} WU exceeds the witness budget of {budget_wu} WU ({stack_bytes}-byte witness stack + 50). \
         The serialized witness stack needs {deficit_wu} more bytes if the program cost stays the same"
    )]
    InsufficientBudget {
        /// Static execution cost of the pruned program, rounded up to whole weight units.
        cost_wu: u64,
        /// Witness budget: the serialized witness stack size plus 50 weight units.
        budget_wu: u64,
        /// Serialized size of the witness stack in bytes.
        stack_bytes: usize,
        /// `cost_wu - budget_wu`, the shortfall in serialized witness size for an unchanged program cost.
        deficit_wu: u64,
    },

    /// Error thrown when compiling the raw Simplicity program source fails.
    #[error("Failed to compile Simplicity program: {0}")]
    Compilation(String),

    /// Error indicating failure while matching or satisfying witness values to the program requirements.
    #[error("Failed to satisfy witness: {0}")]
    WitnessSatisfaction(String),

    /// The program failed. `ExecutionError::JetFailed` is a failed `assert!` or jet,
    /// `ExecutionError::ReachedPrunedBranch` is, for example, `unwrap(None)`, and
    /// `ExecutionError::LimitExceeded` is a program beyond the Bit Machine's limits.
    #[error("Failed to prune program: {0}")]
    Pruning(#[from] simplicityhl::simplicity::bit_machine::ExecutionError),

    /// Error indicating an input that doesn't carry the output it spends.
    #[error("Input {input_index} has no witness UTXO")]
    MissingWitnessUtxo {
        /// The index of the input without a witness UTXO.
        input_index: usize,
    },

    /// Error indicating an input index points past the bounds of available inputs/UTXOs.
    #[error("UTXO index {input_index} out of bounds (have {utxo_count} UTXOs)")]
    UtxoIndexOutOfBounds {
        /// The requested input index to spend.
        input_index: usize,
        /// The actual total number of available UTXOs.
        utxo_count: usize,
    },

    /// The input spends an output whose script isn't the program's.
    #[error("Script pubkey mismatch: expected hash {expected_hash}, got {actual_hash}")]
    ScriptPubkeyMismatch {
        /// The HASH160 of the script of the program's address, not the SHA-256 that
        /// [`Program::get_script_hash`](super::Program::get_script_hash) returns.
        expected_hash: String,
        /// The HASH160 of the script of the spent output.
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

impl ProgramError {
    /// Input `input_index` of `pst` failing to run, with the locktime and sequence it ran with.
    pub(crate) fn program_execution(pst: &PartiallySignedTransaction, input_index: usize, source: Self) -> Self {
        Self::ProgramExecution {
            input_index,
            locktime: pst.locktime().map_or(0, LockTime::to_consensus_u32),
            sequence: pst.inputs()[input_index]
                .sequence
                .map_or(u32::MAX, Sequence::to_consensus_u32),
            source: Box::new(source),
        }
    }
}
