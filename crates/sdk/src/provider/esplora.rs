use crate::error::SimplexError;
use crate::provider::{ProviderAsync, ProviderSync};
pub use simplex_provider::esplora::*;
use simplicityhl::elements::hex::ToHex;
use simplicityhl::elements::{Transaction, Txid};
use std::collections::HashMap;

impl ProviderSync for EsploraClientSync {
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<Txid, SimplexError> {
        self.broadcast_tx(tx).map_err(|e| SimplexError::ProviderError {
            method: "broadcast_tx".to_string(),
            err: e,
        })
    }

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, SimplexError> {
        self.get_fee_estimates().map_err(|e| SimplexError::ProviderError {
            method: "get_fee_estimates".to_string(),
            err: e,
        })
    }

    fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, SimplexError> {
        self.get_tx_elements(&txid.to_hex())
            .map_err(|e| SimplexError::ProviderError {
                method: "get_tx_elements".to_string(),
                err: e,
            })
    }
}

#[async_trait::async_trait]
impl ProviderAsync for EsploraClientAsync {
    async fn broadcast_transaction(&self, tx: &Transaction) -> Result<Txid, SimplexError> {
        self.broadcast_tx(tx).await.map_err(|e| SimplexError::ProviderError {
            method: "broadcast_tx".to_string(),
            err: e,
        })
    }

    async fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, SimplexError> {
        self.get_fee_estimates().await.map_err(|e| SimplexError::ProviderError {
            method: "get_fee_estimates".to_string(),
            err: e,
        })
    }

    async fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, SimplexError> {
        self.get_tx_elements(&txid.to_hex())
            .await
            .map_err(|e| SimplexError::ProviderError {
                method: "get_tx_elements".to_string(),
                err: e,
            })
    }
}
