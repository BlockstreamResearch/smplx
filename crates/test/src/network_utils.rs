use smplx_sdk::provider::{ElementsRpc, EsploraProvider, ProviderError, ProviderTrait};

use crate::error::NetworkUtilsError;

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
}
