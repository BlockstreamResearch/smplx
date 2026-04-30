use smplx_sdk::provider::{ElementsRpc, EsploraProvider, ProviderError, ProviderTrait};
use smplx_sdk::signer::SignerError;

pub struct NetworkUtils {
    rpc: ElementsRpc,
    esplora: EsploraProvider,
}

#[derive(thiserror::Error, Debug)]
pub enum NetworkUtilsError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Signer(#[from] SignerError),
}

impl NetworkUtils {
    pub fn from_context(rpc: ElementsRpc, esplora: EsploraProvider) -> Self {
        Self { rpc, esplora }
    }

    pub fn mine_until_height(&self, target_height: u64) -> Result<(), NetworkUtilsError> {
        self._mine_until_height(target_height)
    }
}

impl NetworkUtils {
    fn _mine_until_height(&self, target_height: u64) -> Result<(), NetworkUtilsError> {
        let current_height = self.rpc.height().map_err(ProviderError::from)?;
        if current_height < target_height {
            let blocks_to_mine = target_height - current_height;
            self.rpc.generate_blocks(blocks_to_mine).map_err(ProviderError::from)?;

            for _ in 0..50 {
                let h = self.esplora.fetch_tip_height()? as u64;
                if h >= target_height {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        Ok(())
    }
}
