use std::io;

#[derive(thiserror::Error, Debug)]
pub enum TestError {
    /// Errors when io error occurred.
    #[error("Occurred io error: '{0}'")]
    Io(#[from] io::Error),

    /// Errors when io error occurred.
    #[error("Occurred config deserialization error: '{0}'")]
    ConfigDeserialize(#[from] toml::de::Error),
}
