mod broadcaster;
mod output_allocator;
mod prevout_resolver;
mod receive_address_provider;
mod session_factory;
mod signer;

use std::sync::Arc;

use lwk_simplicity::error::WalletAbiError;
use lwk_simplicity::wallet_abi::{WalletAbiProvider, WalletAbiProviderBuilder};
use lwk_wollet::elements::OutPoint;
use lwk_wollet::elements::Txid;

use self::broadcaster::WalletAbiBroadcasterAdapter;
use self::output_allocator::WalletAbiOutputAllocatorAdapter;
use self::prevout_resolver::WalletAbiPrevoutResolverAdapter;
use self::receive_address_provider::WalletAbiReceiveAddressProviderAdapter;
use self::session_factory::WalletAbiSessionFactoryAdapter;
use self::signer::WalletAbiSignerAdapter;
use super::state::WalletAbiSharedState;

pub type SimplexWalletAbiProvider = WalletAbiProvider<
    WalletAbiSignerAdapter,
    WalletAbiSessionFactoryAdapter,
    WalletAbiPrevoutResolverAdapter,
    WalletAbiOutputAllocatorAdapter,
    WalletAbiBroadcasterAdapter,
    WalletAbiReceiveAddressProviderAdapter,
>;

#[derive(Debug, thiserror::Error)]
pub enum WalletAbiAdapterError {
    #[error(transparent)]
    SdkSigner(#[from] smplx_sdk::signer::SignerError),

    #[error(transparent)]
    Provider(#[from] smplx_sdk::provider::ProviderError),

    #[error(transparent)]
    Rpc(#[from] smplx_sdk::provider::RpcError),

    #[error(transparent)]
    Unblind(#[from] lwk_wollet::elements::UnblindError),

    #[error("state lock poisoned: {0}")]
    LockPoisoned(&'static str),

    #[error("missing tx output {vout} in transaction {txid}")]
    MissingTxOutput { txid: Txid, vout: u32 },

    #[error("elements rpc is not available for this test context")]
    MissingRpc,

    #[error("wallet-owned output {0} not found")]
    UnknownWalletOutpoint(OutPoint),

    #[error("wallet-abi invariant violation: {0}")]
    Invariant(String),

    #[error("wallet signer x-only key mismatch")]
    XOnlyKeyMismatch,
}

impl From<WalletAbiAdapterError> for WalletAbiError {
    fn from(error: WalletAbiAdapterError) -> Self {
        WalletAbiError::InvalidRequest(error.to_string())
    }
}

#[derive(Clone)]
pub(crate) struct WalletAbiRuntimeAdapters {
    signer_meta: WalletAbiSignerAdapter,
    session_factory: WalletAbiSessionFactoryAdapter,
    prevout_resolver: WalletAbiPrevoutResolverAdapter,
    output_allocator: WalletAbiOutputAllocatorAdapter,
    broadcaster: WalletAbiBroadcasterAdapter,
    receive_address_provider: WalletAbiReceiveAddressProviderAdapter,
}

impl WalletAbiRuntimeAdapters {
    pub(crate) fn new(shared: Arc<WalletAbiSharedState>) -> Self {
        Self {
            signer_meta: WalletAbiSignerAdapter::new(Arc::clone(&shared)),
            session_factory: WalletAbiSessionFactoryAdapter::new(Arc::clone(&shared)),
            prevout_resolver: WalletAbiPrevoutResolverAdapter::new(Arc::clone(&shared)),
            output_allocator: WalletAbiOutputAllocatorAdapter::new(Arc::clone(&shared)),
            broadcaster: WalletAbiBroadcasterAdapter::new(Arc::clone(&shared)),
            receive_address_provider: WalletAbiReceiveAddressProviderAdapter::new(shared),
        }
    }

    pub(crate) fn provider(&self) -> SimplexWalletAbiProvider {
        WalletAbiProviderBuilder::new(
            self.signer_meta.clone(),
            self.session_factory.clone(),
            self.prevout_resolver.clone(),
            self.output_allocator.clone(),
            self.broadcaster.clone(),
            self.receive_address_provider.clone(),
        )
        .build()
    }
}
