use std::sync::Arc;

use lwk_simplicity::wallet_abi::WalletReceiveAddressProvider;
use lwk_wollet::elements::Address;

use super::WalletAbiAdapterError;
use crate::wallet_abi::state::WalletAbiSharedState;

#[derive(Clone)]
pub(crate) struct WalletAbiReceiveAddressProviderAdapter {
    shared: Arc<WalletAbiSharedState>,
}

impl WalletAbiReceiveAddressProviderAdapter {
    pub(crate) fn new(shared: Arc<WalletAbiSharedState>) -> Self {
        Self { shared }
    }
}

impl WalletReceiveAddressProvider for WalletAbiReceiveAddressProviderAdapter {
    type Error = WalletAbiAdapterError;

    fn get_signer_receive_address(&self) -> Result<Address, Self::Error> {
        self.shared.receive_address()
    }
}
