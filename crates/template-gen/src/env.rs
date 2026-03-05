use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use simplex_macros_core::attr::SimfContent;
use simplex_macros_core::attr::codegen::{
    convert_contract_name_to_contract_module, convert_contract_name_to_contract_source_const,
    convert_contract_name_to_struct_name,
};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::{fs, io};

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

    #[error("Failed to find prefix for a file: {0}")]
    NoBasePathForGeneration(#[from] std::path::StripPrefixError),
}

pub struct CodeGenerator {}

struct FileDescriptor {
    simf_content: SimfContent,
    simf_file: PathBuf,
    cwd: PathBuf,
}

type _ContractModName = String;
type _FolderName = String;
type _ContractName = String;

#[derive(Debug, Default)]
struct _TreeNode {
    files: Vec<PathBuf>,
    folders: HashMap<_FolderName, _TreeNode>,
}

impl<'b> CodeGenerator {
    pub fn generate_files(
        cwd: impl AsRef<Path>,
        out_dir: impl AsRef<Path>,
        simfs: &[impl AsRef<Path>],
    ) -> Result<(), CodeGeneratorError> {
        let _ = Self::_generate_files(cwd, out_dir, simfs)?;

        Ok(())
    }

    pub fn generate_artifacts_mod(
        out_dir_name: impl AsRef<str>,
        cwd: impl AsRef<Path>,
        base_path: impl AsRef<Path>,
        out_dir: impl AsRef<Path>,
        simfs: &[impl AsRef<Path>],
    ) -> Result<(), CodeGeneratorError> {
        let out_dir = dbg!(out_dir.as_ref().join(out_dir_name.as_ref()));

        let tree = dbg!(Self::_build_directory_tree(simfs, &base_path)?);
        Self::_generate_tree_file_structure(cwd.as_ref(), &out_dir, tree)?;

        Ok(())
    }
}

impl<'b> CodeGenerator {
    fn _generate_files(
        cwd: impl AsRef<Path>,
        out_dir: impl AsRef<Path>,
        simfs: &[impl AsRef<Path>],
    ) -> Result<Vec<_ContractModName>, CodeGeneratorError> {
        let out_dir = out_dir.as_ref();
        let cwd = cwd.as_ref();

        fs::create_dir_all(out_dir)?;
        let mut module_files = Vec::with_capacity(simfs.len());

        for simf_file_path in simfs {
            let mod_name = Self::_generate_file(cwd, out_dir, simf_file_path)?;
            module_files.push(mod_name);
        }
        Ok(module_files)
    }

    fn _generate_tree_file_structure(
        cwd: &Path,
        out_dir: &Path,
        path_tree: _TreeNode,
    ) -> Result<Vec<_ContractModName>, CodeGeneratorError> {
        let mut mod_filenames = Self::_generate_files(cwd, &out_dir, &path_tree.files)?;
        for (folder_name, tree_node) in path_tree.folders.into_iter() {
            Self::_generate_tree_file_structure(cwd, &out_dir.join(&folder_name), tree_node)?;
            mod_filenames.push(folder_name);
        }
        Self::_generate_mod_rs(&out_dir, &mod_filenames)?;
        Ok(mod_filenames)
    }

    fn _generate_mod_rs(
        out_dir: impl AsRef<Path>,
        simfs_mod_name: &[_ContractModName],
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
        Self::expand_file(code, &mut file)?;
        Ok(())
    }

    fn _build_directory_tree(
        paths: &[impl AsRef<Path>],
        base: impl AsRef<Path>,
    ) -> Result<_TreeNode, CodeGeneratorError> {
        let mut root = _TreeNode::default();

        for path in paths {
            let path = path.as_ref();

            let relative_path = path
                .strip_prefix(base.as_ref())
                .map_err(CodeGeneratorError::NoBasePathForGeneration)?;

            let components: Vec<_> = relative_path
                .components()
                .filter_map(|c| {
                    if let Component::Normal(name) = c {
                        Some(name)
                    } else {
                        None
                    }
                })
                .collect();

            let mut current_node = &mut root;
            let components_len = components.len();

            for (i, name) in components.into_iter().enumerate() {
                let is_file = i == components_len - 1;
                if is_file {
                    current_node.files.push(path.to_path_buf());
                } else {
                    let folder_name = name.to_string_lossy().into_owned();
                    current_node = current_node.folders.entry(folder_name).or_default();
                }
            }
        }

        Ok(root)
    }

    fn _generate_file(
        cwd: impl AsRef<Path>,
        out_dir: impl AsRef<Path>,
        simf_file_path: impl AsRef<Path>,
    ) -> Result<_ContractModName, CodeGeneratorError> {
        let path_buf = PathBuf::from(simf_file_path.as_ref());
        let simf_content =
            SimfContent::extract_content_from_path(&path_buf).map_err(CodeGeneratorError::FailedToExtractContent)?;

        dbg!(cwd.as_ref());
        fs::create_dir_all(&out_dir)?;
        let output_file = out_dir.as_ref().join(format!("{}.rs", &simf_content.contract_name));
        let contract_name = simf_content.contract_name.clone();

        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&output_file)?;
        let code = Self::generate_simf_binding_code(FileDescriptor {
            simf_content,
            simf_file: path_buf,
            cwd: cwd.as_ref().to_path_buf(),
        })?;
        Self::expand_file(code, &mut file)?;
        Ok(contract_name)
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

        let pathdiff = pathdiff::diff_paths(&file_descriptor.simf_file, &file_descriptor.cwd).ok_or(
            CodeGeneratorError::FailedToFindCorrectRelativePath {
                cwd: file_descriptor.cwd,
                simf_file: file_descriptor.simf_file,
            },
        )?;
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
