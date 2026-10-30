use std::io;

use smplx_sdk::provider::{NetworkConvertError, ProviderError};

use smplx_regtest::error::RegtestError;

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

    #[error(transparent)]
    BadNetworkName(#[from] NetworkConvertError),

    #[error("Occurred a network utils execution error: '{0}'")]
    NetworkUtilsExecution(#[from] NetworkUtilsError),
}

#[derive(thiserror::Error, Debug)]
pub enum NetworkUtilsError {
    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error("Unsuccessful action completion, err: '{0}'")]
    UnsuccessfulSync(String),
}
