use std::{cell::RefCell, fmt::Write};

use simplicityhl::{
    debug::DebugSymbols,
    simplicity::node::{Node, Redeem},
    tracker::{DefaultTracker, TrackerLogLevel},
};

thread_local! {
    pub(super) static PROGRAM_LOGGER: RefCell<ProgramLogger> = const { RefCell::new(ProgramLogger { cost_info: None, trace_buffer: Vec::new() }) };
}

/// Helper struct for gathering info from `node.bounds()`.
struct CostInfo {
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

impl ProgramLogger {
    /// Mirrors [`DefaultTracker::with_log_level`] with buffered sinks instead of stderr.
    /// Clears any previously buffered logs before configuring the tracker.
    #[must_use]
    pub fn make_tracker(debug_symbols: &DebugSymbols, log_level: TrackerLogLevel) -> DefaultTracker<'_> {
        Self::clear_logs();

        let tracker = DefaultTracker::new(debug_symbols);

        let tracker = if log_level >= TrackerLogLevel::Debug {
            tracker.with_debug_sink(|label, value| {
                Self::buffer_trace_log(format!("  DBG: {label} = {value}"));
            })
        } else {
            tracker
        };

        let tracker = if log_level >= TrackerLogLevel::Warning {
            tracker.with_warning_sink(|msg| {
                Self::buffer_trace_log(format!("  WARN: {msg}"));
            })
        } else {
            tracker
        };

        if log_level >= TrackerLogLevel::Trace {
            tracker.with_jet_trace_sink(|jet, args, result| {
                let mut msg = format!("  {jet:?}(");

                if let Some(args) = args {
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            msg.push_str(", ");
                        }
                        let _ = write!(msg, "{arg}");
                    }
                } else {
                    msg.push_str("...");
                }

                match result {
                    Some(value) => {
                        let _ = write!(msg, ") = {value}");
                    }
                    None => msg.push_str(") -> [failed]"),
                }

                Self::buffer_trace_log(msg);
            })
        } else {
            tracker
        }
    }

    /// Buffers a line of execution trace output.
    pub fn buffer_trace_log(tracker_logs: String) {
        PROGRAM_LOGGER.with(|logger| logger.borrow_mut().trace_buffer.push(tracker_logs));
    }

    /// Extracts and buffers cost metrics from the given redeem node.
    ///
    /// Overwrites any previously buffered cost info.
    ///
    /// # Safety
    /// Uses `transmute` to extract the inner `u32` from [`Cost`] since no public
    /// accessor exists. Remove once `as_milliweight()` is upstreamed to rust-simplicity.
    pub fn buffer_cost_log(node: &Node<Redeem>) {
        let bounds = node.bounds();
        // FIXME: Cost has no public accessor; remove once as_milliweight() is upstreamed
        let mw: u32 = unsafe { std::mem::transmute(bounds.cost) };
        let encoded = node.to_vec_with_witness();
        let (program_size, witness_size) = (encoded.0.len(), encoded.1.len());
        let cmr_bytes = node.cmr().to_byte_array();

        PROGRAM_LOGGER.with(|logger| {
            logger.borrow_mut().cost_info = Some(CostInfo {
                cmr: std::array::from_fn(|i| cmr_bytes[i]),
                cost: mw / 1000,
                program_size,
                witness_size,
            });
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
                eprintln!("───────────── trace ─────────────");

                for msg in &logger.trace_buffer {
                    eprintln!("{msg}");
                }

                eprintln!();
            }
        });

        Self::clear_logs();
    }

    /// Discards all buffered output without printing.
    pub fn clear_logs() {
        PROGRAM_LOGGER.with(|logger| {
            logger.borrow_mut().cost_info = None;
            logger.borrow_mut().trace_buffer.clear();
        });
    }
}
