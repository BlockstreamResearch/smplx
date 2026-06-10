use simplicityhl::elements::{Address, Script, Transaction, Txid};
use smplx_sdk::provider::{ProviderError, ProviderTrait, SimplicityNetwork};
use smplx_sdk::transaction::{TxReceipt, UTXO};
use std::collections::HashMap;

pub struct MockProvider {
    pub network: SimplicityNetwork,
}

impl MockProvider {
    pub fn new(network: SimplicityNetwork) -> Self {
        Self { network }
    }
}

impl ProviderTrait for MockProvider {
    fn get_network(&self) -> &SimplicityNetwork {
        &self.network
    }

    fn broadcast_transaction(&self, _tx: &Transaction) -> Result<TxReceipt<'_>, ProviderError> {
        unimplemented!("No network access needed for tests")
    }

    fn wait(&self, _txid: &Txid) -> Result<(), ProviderError> {
        unimplemented!("No network access needed for tests")
    }

    fn fetch_tip_height(&self) -> Result<u32, ProviderError> {
        unimplemented!("No network access needed for tests")
    }

    fn fetch_tip_block_hash(&self) -> Result<String, ProviderError> {
        unimplemented!("No network access needed for tests")
    }

    fn fetch_tip_timestamp(&self) -> Result<u64, ProviderError> {
        unimplemented!("No network access needed for tests")
    }

    fn fetch_block_hash_at_height(&self, _block_height: u32) -> Result<String, ProviderError> {
        unimplemented!("No network access needed for tests")
    }

    fn fetch_block_txids(&self, _block_hash: &str) -> Result<Vec<Txid>, ProviderError> {
        unimplemented!("No network access needed for tests")
    }

    fn fetch_transaction(&self, _txid: &Txid) -> Result<Transaction, ProviderError> {
        unimplemented!("No network access needed for tests")
    }

    fn fetch_address_utxos(&self, _address: &Address) -> Result<Vec<UTXO>, ProviderError> {
        unimplemented!("No network access needed for tests")
    }

    fn fetch_scripthash_utxos(&self, _script: &Script) -> Result<Vec<UTXO>, ProviderError> {
        unimplemented!("No network access needed for tests")
    }

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, ProviderError> {
        Ok(HashMap::new())
    }
}
