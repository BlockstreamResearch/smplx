#![warn(clippy::all, clippy::pedantic)]

mod env;
pub use env::CodeGeneratorError;

pub fn expand_simplex_contract_environment(
    outdir: impl AsRef<std::path::Path>,
    simfs: &[impl AsRef<std::path::Path>],
) -> Result<(), CodeGeneratorError> {
    env::CodeGenerator::generate_files(outdir, simfs)
}

pub fn expand_simplex_template(
    outdir: impl AsRef<std::path::Path>,
    simfs: &[impl AsRef<std::path::Path>],
) -> Result<(), CodeGeneratorError> {
    const ARTIFACTS_DIR_NAME: &str = "artifacts";

    env::CodeGenerator::generate_artifacts_mod(ARTIFACTS_DIR_NAME, outdir, simfs)
}
