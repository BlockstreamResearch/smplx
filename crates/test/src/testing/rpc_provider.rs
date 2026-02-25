use simplex_sdk::provider::ProviderError;
use simplex_sdk::provider::ProviderTrait;
use simplicityhl::elements::{Address, OutPoint, Script, Transaction, TxOut, Txid};
use std::collections::HashMap;

pub struct TestRpcProvider {}

impl ProviderTrait for TestRpcProvider {
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<String, ProviderError> {
        todo!()
    }

    fn fetch_transaction(&self, txid: Txid) -> Result<Transaction, ProviderError> {
        todo!()
    }

    fn fetch_address_utxos(&self, address: &Address) -> Result<Vec<(OutPoint, TxOut)>, ProviderError> {
        todo!()
    }

    fn fetch_scripthash_utxos(&self, script: &Script) -> Result<Vec<(OutPoint, TxOut)>, ProviderError> {
        todo!()
    }

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, ProviderError> {
        todo!()
    }
}
