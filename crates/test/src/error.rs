use std::io;

use simplex_sdk::provider::ProviderError;

#[derive(thiserror::Error, Debug)]
pub enum TestError {
    #[error("Occurred io error: '{0}'")]
    Io(#[from] io::Error),

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error("Occurred config deserialization error: '{0}'")]
    ConfigDeserialize(#[from] toml::de::Error),
}
