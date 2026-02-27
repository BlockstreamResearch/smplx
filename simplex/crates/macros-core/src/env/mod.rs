use crate::attr::SimfContent;
use crate::attr::codegen::{
    convert_contract_name_to_contract_module, convert_contract_name_to_contract_source_const,
    convert_contract_name_to_struct_name,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::io::Write;
use std::path::PathBuf;
use std::{env, fs, io};

#[derive(thiserror::Error, Debug)]
pub enum CodeGeneratorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to extract content from path, err: '{0}'")]
    FailedToExtractContent(std::io::Error),

    #[error("Failed to generate file: {0}")]
    GenerationFailed(String),

    #[error(
        "Failed to resolve correct relative path for include_simf! macro, cwd: '{cwd:?}', simf_file: '{simf_file:?}'"
    )]
    FailedToFindCorrectRelativePath { cwd: PathBuf, simf_file: PathBuf },
}

pub struct CodeGenerator {}

struct FileDescriptor {
    simf_content: SimfContent,
    simf_file: PathBuf,
    out_dir: PathBuf,
    cwd: PathBuf,
}

impl<'b> CodeGenerator {
    pub fn generate_files(
        out_dir: impl AsRef<std::path::Path>,
        simfs: &[impl AsRef<std::path::Path>],
    ) -> Result<(), CodeGeneratorError> {
        let out_dir = out_dir.as_ref();

        fs::create_dir_all(out_dir)?;

        for simf_file_path in simfs {
            let path_buf = PathBuf::from(simf_file_path.as_ref());
            let simf_content = SimfContent::extract_content_from_path(&path_buf)
                .map_err(CodeGeneratorError::FailedToExtractContent)?;

            let output_file = out_dir.join(format!("{}.rs", simf_content.contract_name));

            let mut file = fs::OpenOptions::new().write(true).truncate(true).open(&output_file)?;
            Self::expand_file(
                FileDescriptor {
                    simf_content,
                    simf_file: PathBuf::from(simf_file_path.as_ref()),
                    out_dir: PathBuf::from(out_dir),
                    cwd: env::current_dir()?,
                },
                &mut file,
            )?;
        }

        Ok(())
    }

    fn expand_file(file_descriptor: FileDescriptor, buf: &mut dyn Write) -> Result<(), CodeGeneratorError> {
        let code = Self::generate_code(file_descriptor)?;
        let file: syn::File = syn::parse2(code).map_err(|e| CodeGeneratorError::GenerationFailed(e.to_string()))?;
        let prettystr = prettyplease::unparse(&file);
        buf.write_all(prettystr.as_bytes())?;
        buf.flush()?;
        Ok(())
    }

    fn generate_code(file_descriptor: FileDescriptor) -> Result<TokenStream, CodeGeneratorError> {
        let contract_name = &file_descriptor.simf_content.contract_name;
        let program_name = {
            let base_name = convert_contract_name_to_struct_name(contract_name);
            format_ident!("{base_name}Program")
        };
        let include_simf_source_const = convert_contract_name_to_contract_source_const(contract_name);
        let include_simf_module = convert_contract_name_to_contract_module(contract_name);

        let pathdiff = pathdiff::diff_paths(
            &file_descriptor.simf_file.canonicalize().map_err(|e| {
                io::Error::other(format!(
                    "Failed to canonicalize simf file descriptor, '{}', err: '{}'",
                    file_descriptor.simf_file.display(),
                    e
                ))
            })?,
            &file_descriptor.cwd,
        )
        .ok_or(CodeGeneratorError::FailedToFindCorrectRelativePath {
            cwd: file_descriptor.cwd,
            simf_file: file_descriptor.simf_file,
        })?;
        let pathdiff = format!("{}", pathdiff.display());

        let code = quote! {
            use simplex::simplex_macros::include_simf;
            use simplex::simplex_sdk::program::{ArgumentsTrait, Program};
            use simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey;

            pub struct #program_name {
                program: Program,
            }

            impl #program_name {
                pub const SOURCE: &'static str = #include_simf_module::#include_simf_source_const;

                pub fn new(public_key: XOnlyPublicKey, arguments: impl ArgumentsTrait + 'static) -> Self {
                    Self {
                        program: Program::new(Self::SOURCE, public_key, Box::new(arguments)),
                    }
                }

                pub fn get_program(&self) -> &Program {
                    &self.program
                }

                pub fn get_program_mut(&mut self) -> &mut Program {
                    &mut self.program
                }
            }

            include_simf!(#pathdiff);
        };

        Ok(code)
    }
}
