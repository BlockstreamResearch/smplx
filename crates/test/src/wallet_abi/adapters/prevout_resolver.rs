use std::sync::Arc;

use lwk_simplicity::wallet_abi::WalletPrevoutResolver;
use lwk_wollet::bitcoin::PublicKey;
use lwk_wollet::bitcoin::bip32::KeySource;
use lwk_wollet::elements::{OutPoint, TxOut, TxOutSecrets};

use super::WalletAbiAdapterError;
use crate::wallet_abi::state::WalletAbiSharedState;

#[derive(Clone)]
pub(crate) struct WalletAbiPrevoutResolverAdapter {
    shared: Arc<WalletAbiSharedState>,
}

impl WalletAbiPrevoutResolverAdapter {
    pub(crate) fn new(shared: Arc<WalletAbiSharedState>) -> Self {
        Self { shared }
    }
}

impl WalletPrevoutResolver for WalletAbiPrevoutResolverAdapter {
    type Error = WalletAbiAdapterError;

    fn get_bip32_derivation_pair(&self, out_point: &OutPoint) -> Result<Option<(PublicKey, KeySource)>, Self::Error> {
        self.shared.wallet_bip32_pair(out_point)
    }

    fn unblind(&self, tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
        self.shared.unblind_txout(tx_out)
    }

    async fn get_tx_out(&self, outpoint: OutPoint) -> Result<TxOut, Self::Error> {
        self.shared.fetch_tx_out(outpoint)
    }
}
