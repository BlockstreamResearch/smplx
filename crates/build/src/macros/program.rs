use std::sync::Arc;
use std::{error::Error, path::Path};

use proc_macro2::Span;

use simplicityhl::resolution::SourceFile;
use simplicityhl::{AbiMeta, TemplateProgram};

use crate::resolver::load_dependency_map;

use super::parse::SimfContent;

pub fn compile_simf(simf_path: &Path, content: &SimfContent, deps_content: &str) -> syn::Result<AbiMeta> {
    compile_program_inner(simf_path, content, deps_content).map_err(|e| syn::Error::new(Span::call_site(), e))
}

fn compile_program_inner(
    simf_path: &Path,
    content: &SimfContent,
    deps_content: &str,
) -> Result<AbiMeta, Box<dyn Error>> {
    let program = content.content.as_str();
    let source = SourceFile::new(simf_path, Arc::from(program));
    let dependency_map = load_dependency_map(deps_content)?;

    Ok(TemplateProgram::new_with_dep(source, &dependency_map)?.generate_abi_meta()?)
}
