use std::io;

#[derive(thiserror::Error, Debug)]
pub enum TestError {
    #[error("Occurred io error: '{0}'")]
    Io(#[from] io::Error),

    #[error("Occurred config deserialization error: '{0}'")]
    ConfigDeserialize(#[from] toml::de::Error),
}
