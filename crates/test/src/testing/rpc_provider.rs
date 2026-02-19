use simplex_sdk::error::SimplexError;
use simplex_sdk::provider::Provider;
use simplicityhl::elements::{Transaction, Txid};
use std::collections::HashMap;

pub struct TestRpcProvider {}

impl Provider for TestRpcProvider {
    fn broadcast_transaction(&self, tx: &Transaction) -> Result<String, SimplexError> {
        todo!()
    }

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, SimplexError> {
        todo!()
    }

    fn fetch_transaction(&self, txid: Txid) -> Result<Transaction, SimplexError> {
        todo!()
    }
}
