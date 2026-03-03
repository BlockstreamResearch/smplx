use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use simplex_macros_core::attr::SimfContent;
use simplex_macros_core::attr::codegen::{
    convert_contract_name_to_contract_module, convert_contract_name_to_contract_source_const,
    convert_contract_name_to_struct_name,
};
use std::io::Write;
use std::path::PathBuf;
use std::{env, fs, io};

#[derive(thiserror::Error, Debug)]
pub enum CodeGeneratorError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to extract content from path, err: '{0}'")]
    FailedToExtractContent(io::Error),

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
    cwd: PathBuf,
}

type ContractModName = String;

impl<'b> CodeGenerator {
    pub fn generate_files(
        out_dir: impl AsRef<std::path::Path>,
        simfs: &[impl AsRef<std::path::Path>],
    ) -> Result<(), CodeGeneratorError> {
        let _ = Self::_generate_files(out_dir, simfs)?;

        Ok(())
    }

    fn _generate_files(
        out_dir: impl AsRef<std::path::Path>,
        simfs: &[impl AsRef<std::path::Path>],
    ) -> Result<Vec<ContractModName>, CodeGeneratorError> {
        let out_dir = out_dir.as_ref();

        fs::create_dir_all(out_dir)?;
        let mut module_files = Vec::with_capacity(simfs.len());

        for simf_file_path in simfs {
            let path_buf = PathBuf::from(simf_file_path.as_ref());
            let simf_content = SimfContent::extract_content_from_path(&path_buf)
                .map_err(CodeGeneratorError::FailedToExtractContent)?;

            let output_file = out_dir.join(format!("{}.rs", simf_content.contract_name));
            module_files.push(simf_content.contract_name.clone());

            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&output_file)?;
            let code = Self::generate_simf_binding_code(FileDescriptor {
                simf_content,
                simf_file: PathBuf::from(simf_file_path.as_ref()),
                cwd: env::current_dir()?,
            })?;
            Self::expand_file(code, &mut file)?;
        }
        Ok(module_files)
    }

    pub fn generate_artifacts_mod(
        out_dir_name: impl AsRef<str>,
        out_dir: impl AsRef<std::path::Path>,
        simfs: &[impl AsRef<std::path::Path>],
    ) -> Result<(), CodeGeneratorError> {
        let out_dir = out_dir.as_ref();
        let out_dir = dbg!(out_dir.join(out_dir_name.as_ref()));
        let mod_filenames = Self::_generate_files(&out_dir, simfs)?;
        Self::_generate_mod_rs(&out_dir, &mod_filenames)?;

        Ok(())
    }

    pub fn _generate_mod_rs(
        out_dir: impl AsRef<std::path::Path>,
        simfs_mod_name: &[ContractModName],
    ) -> Result<(), CodeGeneratorError> {
        let out_dir = out_dir.as_ref();
        let output_file = out_dir.join("mod.rs");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&output_file)?;
        let simfs_mod_name = simfs_mod_name.iter().map(|x| format_ident!("{x}")).collect::<Vec<_>>();
        let code = quote! {
            #(pub mod #simfs_mod_name);*;
        };
        dbg!(code.to_string());
        Self::expand_file(code, &mut file)?;
        Ok(())
    }

    fn expand_file(code: TokenStream, buf: &mut dyn Write) -> Result<(), CodeGeneratorError> {
        let file: syn::File = syn::parse2(code).map_err(|e| CodeGeneratorError::GenerationFailed(e.to_string()))?;
        let prettystr = prettyplease::unparse(&file);
        buf.write_all(prettystr.as_bytes())?;
        buf.flush()?;
        Ok(())
    }

    fn generate_simf_binding_code(file_descriptor: FileDescriptor) -> Result<TokenStream, CodeGeneratorError> {
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
