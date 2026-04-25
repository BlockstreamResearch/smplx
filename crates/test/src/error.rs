use std::io;

use lwk_simplicity::error::WalletAbiError;
use smplx_sdk::provider::{ProviderError, RpcError};
use smplx_sdk::signer::SignerError;

use smplx_regtest::error::RegtestError;

use crate::wallet_abi::WalletAbiAdapterError;

#[derive(thiserror::Error, Debug)]
pub enum TestError {
    #[error(transparent)]
    Regtest(#[from] RegtestError),

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error("Failed to deserialize config: '{0}'")]
    ConfigDeserialize(#[from] toml::de::Error),

    #[error("io error occurred: '{0}'")]
    Io(#[from] io::Error),

    #[error("Network name should either be `Liquid`, `LiquidTestnet` or `ElementsRegtest`, got: {0}")]
    BadNetworkName(String),

    #[error(transparent)]
    SdkSigner(#[from] SignerError),

    #[error("wallet-abi invariant violation: {0}")]
    WalletAbiInvariant(String),

    #[error(transparent)]
    WalletAbi(#[from] WalletAbiError),

    #[error(transparent)]
    WalletAbiAdapter(#[from] WalletAbiAdapterError),

    #[error(transparent)]
    WalletAbiRpc(#[from] RpcError),
}
