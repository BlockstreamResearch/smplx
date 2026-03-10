#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error(transparent)]
    Signer(#[from] simplex_sdk::signer::SignerError),

    #[error(transparent)]
    Regtest(#[from] simplex_regtest::error::RegtestError),

    #[error(transparent)]
    Test(#[from] simplex_test::error::TestError),

    #[error(transparent)]
    Build(#[from] simplex_build::error::BuildError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
