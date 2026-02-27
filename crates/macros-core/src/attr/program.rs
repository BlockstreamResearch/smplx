use crate::attr::parse::SimfContent;
use proc_macro2::Span;
use simplicityhl::{AbiMeta, TemplateProgram};
use std::error::Error;

pub fn compile_simf(content: &SimfContent) -> syn::Result<AbiMeta> {
    compile_program_inner(content).map_err(|e| syn::Error::new(Span::call_site(), e))
}

fn compile_program_inner(content: &SimfContent) -> Result<AbiMeta, Box<dyn Error>> {
    let program = content.content.as_str();
    Ok(TemplateProgram::new(program)?.generate_abi_meta()?)
}
