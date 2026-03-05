#![warn(clippy::all, clippy::pedantic)]

mod env;
pub use env::CodeGeneratorError;

pub fn expand_only_files(
    cwd: impl AsRef<std::path::Path>,
    outdir: impl AsRef<std::path::Path>,
    simfs: &[impl AsRef<std::path::Path>],
) -> Result<(), CodeGeneratorError> {
    env::CodeGenerator::generate_files(cwd, outdir, simfs)
}

pub fn expand_files_with_nested_dirs(
    cwd: impl AsRef<std::path::Path>,
    base_dir: impl AsRef<std::path::Path>,
    outdir: impl AsRef<std::path::Path>,
    simfs: &[impl AsRef<std::path::Path>],
) -> Result<(), CodeGeneratorError> {
    const ARTIFACTS_DIR_NAME: &str = "artifacts";

    env::CodeGenerator::generate_artifacts_mod(ARTIFACTS_DIR_NAME, cwd, base_dir, outdir, simfs)
}
