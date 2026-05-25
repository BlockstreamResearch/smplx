use std::{cell::RefCell, fmt::Write};
/// Helper struct for gathering info from `node.bounds()`
pub struct CostInfo {
    /// truncated cmr
    pub cmr: [u8; 8],
    /// inner value of `Cost`
    pub cost: u32,
    /// program size in bytes
    pub program_size: usize,
    /// witness size in bytes
    pub witness_size: usize,
}

/// Logger state for buffering program execution output.
///
/// Buffers are flushed to stderr only on successful transaction finalization,
/// discarding output from intermediate estimation passes.
pub struct ProgramLogger {
    /// Cost metrics from the most recent program execution.
    cost_info: Option<CostInfo>,
    /// Execution trace lines in insertion order.
    trace_buffer: Vec<String>,
}

thread_local! {
    static PROGRAM_LOGGER: RefCell<ProgramLogger> = const { RefCell::new(ProgramLogger { cost_info: None, trace_buffer: Vec::new() }) };

}

/// Buffers a line of execution trace output.
pub fn buffer_trace_log(tracker_logs: String) {
    PROGRAM_LOGGER.with(|logger| logger.borrow_mut().trace_buffer.push(tracker_logs));
}

/// Buffers a cost log entry for the given program source.
///
/// Overwrites any previous entry for the same source, ensuring only
/// the most recent execution's cost is reported.
pub fn buffer_cost_log(stats: CostInfo) {
    PROGRAM_LOGGER.with(|logger| {
        logger.borrow_mut().cost_info = Some(stats);
    });
}
/// Flushes all buffered cost and trace output to stderr, then clears buffers.
///
/// Cost metrics are printed first, followed by execution trace lines in
/// insertion order.
///
/// Call this on successful transaction finalization.
pub fn flush_logs() {
    PROGRAM_LOGGER.with(|logger| {
        let logger = logger.borrow();

        if let Some(cost_info) = &logger.cost_info {
            let cmr_hex: String = cost_info.cmr.iter().fold(String::new(), |mut output, b| {
                let _ = write!(output, "{b:02x}");
                output
            });

            eprintln!(
                "Program info: cmr=[{}] cost={}wu  prog={}b  witness={}b",
                cmr_hex, cost_info.cost, cost_info.program_size, cost_info.witness_size,
            );
        }

        if !logger.trace_buffer.is_empty() {
            eprintln!("── trace ──────────────────");
            for msg in &logger.trace_buffer {
                eprintln!("{msg}");
            }
        }
    });

    clear_logs();
}

/// Discards all buffered output without printing.
pub fn clear_logs() {
    PROGRAM_LOGGER.with(|logger| {
        logger.borrow_mut().cost_info = None;
        logger.borrow_mut().trace_buffer.clear();
    });
}
