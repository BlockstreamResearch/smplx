pub type CommandResult<T> = Result<T, CommandError>;

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error(transparent)]
    Signer(#[from] smplx_sdk::signer::SignerError),

    #[error(transparent)]
    Regtest(#[from] smplx_regtest::error::RegtestError),

    #[error(transparent)]
    Test(#[from] smplx_test::error::TestError),

    #[error(transparent)]
    Build(#[from] smplx_build::error::BuildError),

    #[error(transparent)]
    Clean(#[from] crate::commands::clean::CleanError),

    #[error(transparent)]
    Init(#[from] crate::commands::init::InitError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
