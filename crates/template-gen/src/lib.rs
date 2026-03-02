#![warn(clippy::all, clippy::pedantic)]

mod env;
pub use env::*;

pub fn expand_simplex_contract_enviroment(
    outdir: impl AsRef<std::path::Path>,
    simfs: &[impl AsRef<std::path::Path>],
) -> Result<(), CodeGeneratorError> {
    CodeGenerator::generate_files(outdir, simfs)
}

pub fn expand_simplex_template(
    outdir: impl AsRef<std::path::Path>,
    simfs: &[impl AsRef<std::path::Path>],
) -> Result<(), CodeGeneratorError> {
    CodeGenerator::generate_template_lib(outdir, simfs)
}
