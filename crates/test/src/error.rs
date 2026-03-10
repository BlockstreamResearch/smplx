use std::io;

use simplex_sdk::provider::ProviderError;
use simplex_sdk::signer::SignerError;

use simplex_regtest::error::RegtestError;

#[derive(thiserror::Error, Debug)]
pub enum TestError {
    #[error(transparent)]
    Regtest(#[from] RegtestError),
    
    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error(transparent)]
    Signer(#[from] SignerError),
    
    #[error("Failed to deserialize config: '{0}'")]
    ConfigDeserialize(#[from] toml::de::Error),

    #[error("io error occurred: '{0}'")]
    Io(#[from] io::Error),
}
