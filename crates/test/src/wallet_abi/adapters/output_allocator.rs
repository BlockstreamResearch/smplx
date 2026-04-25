use std::sync::Arc;

use lwk_simplicity::wallet_abi::{
    WalletOutputAllocator, WalletOutputRequest, WalletOutputTemplate, WalletRequestSession,
};

use super::WalletAbiAdapterError;
use crate::wallet_abi::state::WalletAbiSharedState;

#[derive(Clone)]
pub(crate) struct WalletAbiOutputAllocatorAdapter {
    shared: Arc<WalletAbiSharedState>,
}

impl WalletAbiOutputAllocatorAdapter {
    pub(crate) fn new(shared: Arc<WalletAbiSharedState>) -> Self {
        Self { shared }
    }
}

impl WalletOutputAllocator for WalletAbiOutputAllocatorAdapter {
    type Error = WalletAbiAdapterError;

    fn get_wallet_output_template(
        &self,
        _session: &WalletRequestSession,
        request: &WalletOutputRequest,
    ) -> Result<WalletOutputTemplate, Self::Error> {
        self.shared.output_template(request)
    }
}
