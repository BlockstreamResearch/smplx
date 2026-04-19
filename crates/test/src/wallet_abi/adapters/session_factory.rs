use std::sync::Arc;

use lwk_simplicity::wallet_abi::{WalletRequestSession, WalletSessionFactory};

use super::WalletAbiAdapterError;
use crate::wallet_abi::state::WalletAbiSharedState;

#[derive(Clone)]
pub(crate) struct WalletAbiSessionFactoryAdapter {
    shared: Arc<WalletAbiSharedState>,
}

impl WalletAbiSessionFactoryAdapter {
    pub(crate) fn new(shared: Arc<WalletAbiSharedState>) -> Self {
        Self { shared }
    }

    pub(crate) fn open_wallet_request_session_sync(&self) -> Result<WalletRequestSession, WalletAbiAdapterError> {
        self.shared.open_wallet_request_session()
    }
}

impl WalletSessionFactory for WalletAbiSessionFactoryAdapter {
    type Error = WalletAbiAdapterError;

    async fn open_wallet_request_session(&self) -> Result<WalletRequestSession, Self::Error> {
        self.open_wallet_request_session_sync()
    }
}
