#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("Failed to terminate elements")]
    ElementsTermination(),

    #[error("Failed to terminate electrs")]
    ElectrsTermination(),
}
