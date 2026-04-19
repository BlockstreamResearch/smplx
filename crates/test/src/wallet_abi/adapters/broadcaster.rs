use std::future::Future;
use std::sync::Arc;

use lwk_simplicity::wallet_abi::WalletBroadcaster;
use lwk_wollet::elements::{Transaction, Txid};

use super::WalletAbiAdapterError;
use crate::wallet_abi::state::WalletAbiSharedState;

#[derive(Clone)]
pub(crate) struct WalletAbiBroadcasterAdapter {
    shared: Arc<WalletAbiSharedState>,
}

impl WalletAbiBroadcasterAdapter {
    pub(crate) fn new(shared: Arc<WalletAbiSharedState>) -> Self {
        Self { shared }
    }
}

impl WalletBroadcaster for WalletAbiBroadcasterAdapter {
    type Error = WalletAbiAdapterError;

    fn broadcast_transaction(&self, tx: &Transaction) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
        let tx = tx.clone();
        async move { self.shared.broadcast(&tx) }
    }
}
