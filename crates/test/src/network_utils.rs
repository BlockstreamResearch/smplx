use std::time::SystemTime;

use smplx_sdk::provider::{ElementsRpc, EsploraProvider, GetBlockchainInfo, ProviderError, ProviderTrait};

use crate::error::{NetworkUtilsError, TimeShiftError};

pub struct NetworkUtils {
    rpc: ElementsRpc,
    esplora: EsploraProvider,
}

impl NetworkUtils {
    pub fn new(rpc: ElementsRpc, esplora: EsploraProvider) -> Self {
        Self { rpc, esplora }
    }

    pub fn mine_until_height(&self, target_height: u64) -> Result<(), NetworkUtilsError> {
        let current_height = self.rpc.height().map_err(ProviderError::from)?;

        if current_height < target_height {
            let blocks_to_mine = target_height - current_height;

            self.rpc.generate_blocks(blocks_to_mine).map_err(ProviderError::from)?;

            let mut h = 0;
            for _ in 0..50 {
                h = self.esplora.fetch_tip_height()? as u64;

                if h >= target_height {
                    return Ok(());
                }

                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            return Err(NetworkUtilsError::UnsuccessfulSync(format!(
                "Failed to complete mining until height, got: '{h}', desired height: '{current_height}'",
            )));
        }

        Ok(())
    }

    pub fn get_blockchain_info(&self) -> Result<GetBlockchainInfo, NetworkUtilsError> {
        let blockchain_info = self.rpc.get_blockchain_info().map_err(ProviderError::from)?;
        Ok(blockchain_info)
    }
}

// Mock-time utilities for mockable Elements regtest nodes.
impl NetworkUtils {
    /// Minimum mock timestamp accepted by Simplex for a fresh Liquid regtest chain.
    ///
    /// Liquid regtest's genesis timestamp is [`1_296_688_602`][liquid_genesis].
    /// Simplex does not accept a nonzero mock timestamp earlier than genesis.
    ///
    /// [liquid_genesis]: https://github.com/ElementsProject/elements/blob/elements-23.3.1/src/chainparams.cpp#L1207-L1210
    pub const MIN_MINEABLE_MOCK_TIME: u64 = 1_296_688_602;

    /// Sentinel passed to Elements to disable mock time.
    ///
    /// When mock time is disabled, Elements reads the node host's system clock
    /// whenever time is queried, so its internal time advances normally.
    pub const RESET_TIMESTAMP: u64 = 0;

    /// Number of blocks normally needed to move median time past to a later timestamp.
    ///
    /// Elements calculates MTP as the median of the tip and its ten ancestors.
    /// On an established chain with an eleven-block window, [`Self::BLOCKS_TO_ADVANCE_MTP`]
    /// blocks carrying the target timestamp form a majority. This assumes the target is late
    /// enough for the miner to stamp all [`Self::BLOCKS_TO_ADVANCE_MTP`] blocks with that value.
    ///
    /// See the Elements [MTP implementation][mtp].
    ///
    /// [mtp]: https://github.com/ElementsProject/elements/blob/elements-23.3.1/src/chain.h#L364-L381
    pub const BLOCKS_TO_ADVANCE_MTP: u64 = 6;

    /// Returns the current chain tip's median-time-past (MTP).
    ///
    /// MTP is derived from block timestamps. Advancing either real time or the
    /// node's mocked clock does not change it until new blocks are accepted.
    pub fn get_mtp(&self) -> Result<u64, NetworkUtilsError> {
        let mtp = self.rpc.get_blockchain_info().map_err(ProviderError::from)?.median_time;
        Ok(u64::try_from(mtp).map_err(|_| TimeShiftError::InvalidMedianTime { current_mtp: mtp })?)
    }

    /// Sets the node's mocked clock without mining any blocks.
    ///
    /// This is the low-level operation. It does not apply the Simplex timestamp
    /// policy or mine blocks. A value of [`Self::RESET_TIMESTAMP`] disables mock
    /// time and makes Elements use the node host's system clock.
    ///
    /// Use [`Self::set_mock_time`] when the chain's MTP should be advanced safely.
    ///
    /// # Errors
    ///
    /// Returns an error when the RPC call fails.
    pub fn set_mock_time_manual(&self, timestamp: u64) -> Result<(), NetworkUtilsError> {
        Ok(self.rpc.set_mock_time(timestamp).map_err(ProviderError::from)?)
    }

    /// Sets mock time and advances MTP, or restores the use of system time when `timestamp`
    /// is [`Self::RESET_TIMESTAMP`].
    ///
    /// A nonzero target must be strictly greater than the current MTP. The method
    /// sets Elements' mocked clock, mines [`Self::BLOCKS_TO_ADVANCE_MTP`] blocks,
    /// and verifies that the resulting MTP equals the target.
    ///
    /// This method cannot move MTP backward. Tests that need an earlier MTP
    /// must start a fresh chain near that time. Passing [`Self::RESET_TIMESTAMP`]
    /// delegates to [`Self::reset_mock_time`].
    ///
    /// # Errors
    ///
    /// Returns an error if validation, an RPC or mining operation, synchronization,
    /// or final MTP verification fails.
    pub fn set_mock_time(&self, timestamp: u64) -> Result<(), NetworkUtilsError> {
        Self::validate_mock_time(timestamp)?;

        if timestamp == 0 {
            return self.reset_mock_time();
        }

        let current_mtp = self.get_mtp()?;
        Self::validate_forward_shift(current_mtp, timestamp)?;

        self.advance_mtp(timestamp)?;

        let actual_mtp = self.get_mtp()?;
        if actual_mtp != timestamp {
            return Err(TimeShiftError::UnexpectedMedianTime {
                expected: timestamp,
                actual: actual_mtp,
            }
            .into());
        }

        Ok(())
    }

    /// Disables mock time and advances MTP toward the node host's system time.
    ///
    /// Before changing the node, this method verifies that the calling process's
    /// system time is later than the current MTP. It then sends
    /// [`Self::RESET_TIMESTAMP`] to Elements and mines
    /// [`Self::BLOCKS_TO_ADVANCE_MTP`] blocks using the node host's system clock.
    ///
    /// For the internal Simplex regtest node, the calling process and node normally
    /// share a system clock. A custom or remote RPC node may use a different clock.
    ///
    /// # Errors
    ///
    /// Returns an error if system-clock validation, an RPC or mining operation,
    /// or synchronization fails.
    pub fn reset_mock_time(&self) -> Result<(), NetworkUtilsError> {
        let possible_new_timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(TimeShiftError::from)?
            .as_secs();

        let current_mtp = self.get_mtp()?;
        Self::validate_forward_shift(current_mtp, possible_new_timestamp)?;

        self.advance_mtp(Self::RESET_TIMESTAMP)?;

        Ok(())
    }

    fn advance_mtp(&self, timestamp: u64) -> Result<(), NetworkUtilsError> {
        let current_height = self.rpc.height().map_err(ProviderError::from)?;
        let target_height = current_height
            .checked_add(Self::BLOCKS_TO_ADVANCE_MTP)
            .ok_or(TimeShiftError::BlockHeightOverflow { current_height })?;

        self.set_mock_time_manual(timestamp)?;
        self.mine_until_height(target_height)?;

        Ok(())
    }

    /// Validates a mock timestamp independently of the current chain state.
    ///
    /// This applies the static Simplex policy without querying a node.
    /// Acceptable values are:
    ///
    /// - [`Self::RESET_TIMESTAMP`], which disables mock time;
    /// - values greater than or equal to [`Self::MIN_MINEABLE_MOCK_TIME`].
    ///
    /// This helper rejects nonzero values below [`Self::MIN_MINEABLE_MOCK_TIME`]
    /// to prevent the mocked clock from starting before the default Liquid regtest
    /// genesis timestamp. Elements' `setmocktime` RPC itself accepts any non-negative
    /// signed integer.
    fn validate_mock_time(timestamp: u64) -> Result<(), TimeShiftError> {
        if timestamp == 0 || timestamp >= Self::MIN_MINEABLE_MOCK_TIME {
            return Ok(());
        }

        Err(TimeShiftError::InvalidMockTime {
            timestamp,
            minimum: Self::MIN_MINEABLE_MOCK_TIME,
        })
    }

    /// Enforces the Simplex policy that runtime MTP shifts move only forward.
    ///
    /// Elements permits moving its mocked clock backward, but doing so cannot lower
    /// the chain's MTP and may prevent subsequent blocks from being mined.
    #[inline]
    fn validate_forward_shift(current_mtp: u64, timestamp: u64) -> Result<(), TimeShiftError> {
        if timestamp <= current_mtp {
            return Err(TimeShiftError::NotForward { current_mtp, timestamp });
        }

        Ok(())
    }
}
