use std::io;

use smplx_sdk::provider::ProviderError;

use smplx_regtest::error::RegtestError;

use crate::fuzz::builders::ProgramTarget;

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

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum FuzzError {
    #[error("At least one program target is required")]
    NoProgramTargets,

    #[error("Duplicate program target: {0:?}")]
    DuplicateProgramTarget(ProgramTarget),

    #[error("Program input target index {index} is out of bounds for {input_count} inputs")]
    InputTargetOutOfBounds { index: usize, input_count: usize },

    #[error("Program output target index {index} is out of bounds for {output_count} outputs")]
    OutputTargetOutOfBounds { index: usize, output_count: usize },

    #[error("Fuzz transaction post hook failed: {0}")]
    PostHook(String),
}
