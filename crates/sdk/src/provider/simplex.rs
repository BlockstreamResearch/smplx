use std::collections::HashMap;

use electrsd::bitcoind::bitcoincore_rpc::Auth;

use simplicityhl::elements::{Address, OutPoint, Script, Transaction, TxOut, Txid};

use crate::provider::SimplicityNetwork;

use super::error::ProviderError;
use super::provider::ProviderTrait;

use super::{ElementsRpc, EsploraProvider};

pub struct SimplexProvider {
    pub esplora: EsploraProvider,
    pub elements: ElementsRpc,
}

impl SimplexProvider {
    pub fn new(
        esplora_url: String,
        elements_url: String,
        auth: Auth,
        network: SimplicityNetwork,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            esplora: EsploraProvider::new(esplora_url, network),
            elements: ElementsRpc::new(elements_url, auth)?,
        })
    }
}

impl ProviderTrait for SimplexProvider {
    fn get_network(&self) -> &SimplicityNetwork {
        self.esplora.get_network()
    }

    fn broadcast_transaction(&self, tx: &Transaction) -> Result<Txid, ProviderError> {
        let txid = self.esplora.broadcast_transaction(tx)?;

        self.elements.generate_blocks(1)?;

        Ok(txid)
    }

    fn wait(&self, txid: &Txid) -> Result<(), ProviderError> {
        Ok(self.esplora.wait(txid)?)
    }

    fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, ProviderError> {
        Ok(self.esplora.fetch_transaction(txid)?)
    }

    fn fetch_address_utxos(&self, address: &Address) -> Result<Vec<(OutPoint, TxOut)>, ProviderError> {
        Ok(self.esplora.fetch_address_utxos(address)?)
    }

    fn fetch_scripthash_utxos(&self, script: &Script) -> Result<Vec<(OutPoint, TxOut)>, ProviderError> {
        Ok(self.esplora.fetch_scripthash_utxos(script)?)
    }

    fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, ProviderError> {
        Ok(self.esplora.fetch_fee_estimates()?)
    }
}
