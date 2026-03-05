use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use simplex_macros_core::attr::SimfContent;
use simplex_macros_core::attr::codegen::{
    convert_contract_name_to_contract_module, convert_contract_name_to_contract_source_const,
    convert_contract_name_to_struct_name,
};

use super::error::BuildError;

pub struct ArtifactsGenerator {}

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

impl ArtifactsGenerator {
    pub fn generate_artifacts(
        out_dir: impl AsRef<str>,
        base_dir: impl AsRef<str>,
        simfs: &[impl AsRef<PathBuf>],
    ) -> Result<(), BuildError> {
        // let tree = dbg!(Self::build_directory_tree(simfs, &base_path)?);

        // Self::generate_tree_file_structure(cwd.as_ref(), &out_dir, tree)?;

        Ok(())
    }

    fn build_directory_tree(paths: &[impl AsRef<Path>], base: impl AsRef<Path>) -> Result<_TreeNode, BuildError> {
        let mut root = _TreeNode::default();

        for path in paths {
            let path = path.as_ref();

            let relative_path = path
                .strip_prefix(base.as_ref())
                .map_err(BuildError::NoBasePathForGeneration)?;

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

    fn generate_files(
        cwd: impl AsRef<Path>,
        out_dir: impl AsRef<Path>,
        simfs: &[impl AsRef<Path>],
    ) -> Result<Vec<_ContractModName>, BuildError> {
        let out_dir = out_dir.as_ref();
        let cwd = cwd.as_ref();

        fs::create_dir_all(out_dir)?;
        let mut module_files = Vec::with_capacity(simfs.len());

        for simf_file_path in simfs {
            let mod_name = Self::generate_file(cwd, out_dir, simf_file_path)?;
            module_files.push(mod_name);
        }
        Ok(module_files)
    }

    fn generate_tree_file_structure(
        cwd: &Path,
        out_dir: &Path,
        path_tree: _TreeNode,
    ) -> Result<Vec<_ContractModName>, BuildError> {
        let mut mod_filenames = Self::generate_files(cwd, &out_dir, &path_tree.files)?;
        for (folder_name, tree_node) in path_tree.folders.into_iter() {
            Self::generate_tree_file_structure(cwd, &out_dir.join(&folder_name), tree_node)?;
            mod_filenames.push(folder_name);
        }
        Self::generate_mod_rs(&out_dir, &mod_filenames)?;
        Ok(mod_filenames)
    }

    fn generate_mod_rs(out_dir: impl AsRef<Path>, simfs_mod_name: &[_ContractModName]) -> Result<(), BuildError> {
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

    fn generate_file(
        cwd: impl AsRef<Path>,
        out_dir: impl AsRef<Path>,
        simf_file_path: impl AsRef<Path>,
    ) -> Result<_ContractModName, BuildError> {
        let path_buf = PathBuf::from(simf_file_path.as_ref());
        let simf_content =
            SimfContent::extract_content_from_path(&path_buf).map_err(BuildError::FailedToExtractContent)?;

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

    fn expand_file(code: TokenStream, buf: &mut dyn Write) -> Result<(), BuildError> {
        let file: syn::File = syn::parse2(code).map_err(|e| BuildError::GenerationFailed(e.to_string()))?;
        let prettystr = prettyplease::unparse(&file);
        buf.write_all(prettystr.as_bytes())?;
        buf.flush()?;
        Ok(())
    }

    fn generate_simf_binding_code(file_descriptor: FileDescriptor) -> Result<TokenStream, BuildError> {
        let contract_name = &file_descriptor.simf_content.contract_name;
        let program_name = {
            let base_name = convert_contract_name_to_struct_name(contract_name);
            format_ident!("{base_name}Program")
        };
        let include_simf_source_const = convert_contract_name_to_contract_source_const(contract_name);
        let include_simf_module = convert_contract_name_to_contract_module(contract_name);

        let pathdiff = pathdiff::diff_paths(&file_descriptor.simf_file, &file_descriptor.cwd).ok_or(
            BuildError::FailedToFindCorrectRelativePath {
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
