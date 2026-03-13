use std::io;

use simplex_sdk::provider::ProviderError;
use simplex_sdk::provider::RpcError;
use simplex_sdk::signer::SignerError;

#[derive(thiserror::Error, Debug)]
pub enum RegtestError {
    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error(transparent)]
    Rpc(#[from] RpcError),

    #[error(transparent)]
    Signer(#[from] SignerError),

    #[error("Failed to terminate elements")]
    ElementsTermination(),

    #[error("Failed to terminate electrs")]
    ElectrsTermination(),

    #[error("Failed to deserialize config: '{0}'")]
    ConfigDeserialize(#[from] toml::de::Error),

    #[error("io error occurred: '{0}'")]
    Io(#[from] io::Error),
}
