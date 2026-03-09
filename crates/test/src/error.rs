use std::io;

use simplex_sdk::provider::ProviderError;
use simplex_sdk::provider::RpcError;
use simplex_sdk::signer::SignerError;

#[derive(thiserror::Error, Debug)]
pub enum TestError {
    #[error("Occurred io error: '{0}'")]
    Io(#[from] io::Error),

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error(transparent)]
    Rpc(#[from] RpcError),

    #[error(transparent)]
    Signer(#[from] SignerError),

    #[error("Occurred config deserialization error: '{0}'")]
    ConfigDeserialize(#[from] toml::de::Error),
}
