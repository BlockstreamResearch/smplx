#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Client(#[from] simplex_regtest::error::ClientError),

    #[error(transparent)]
    Test(#[from] simplex_test::error::TestError),

    #[error(transparent)]
    Build(#[from] simplex_build::error::BuildError),
}
