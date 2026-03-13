use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::macros::codegen::{
    convert_contract_name_to_contract_module, convert_contract_name_to_contract_source_const,
    convert_contract_name_to_struct_name,
};
use crate::macros::parse::SimfContent;

use super::error::BuildError;

pub struct ArtifactsGenerator {}

#[derive(Default)]
struct TreeNode {
    files: Vec<PathBuf>,
    dirs: HashMap<String, TreeNode>,
}

impl ArtifactsGenerator {
    pub fn generate_artifacts(
        out_dir: impl AsRef<Path>,
        base_dir: impl AsRef<Path>,
        simfs: &[impl AsRef<Path>],
    ) -> Result<(), BuildError> {
        let tree = Self::build_directory_tree(&base_dir, simfs)?;

        Self::generate_bindings(out_dir.as_ref(), tree)?;

        Ok(())
    }

    fn build_directory_tree(base_dir: impl AsRef<Path>, paths: &[impl AsRef<Path>]) -> Result<TreeNode, BuildError> {
        let mut root = TreeNode::default();

        for path in paths {
            let path = path.as_ref();

            let relative_path = path
                .strip_prefix(base_dir.as_ref())
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
                    let dir_name = name.to_string_lossy().into_owned();

                    current_node = current_node.dirs.entry(dir_name).or_default();
                }
            }
        }

        Ok(root)
    }

    fn generate_bindings(out_dir: &Path, path_tree: TreeNode) -> Result<Vec<String>, BuildError> {
        fs::create_dir_all(out_dir)?;

        let mut mod_filenames = Self::generate_simfs(&out_dir, &path_tree.files)?;

        for (dir_name, tree_node) in path_tree.dirs.into_iter() {
            Self::generate_bindings(&out_dir.join(&dir_name), tree_node)?;
            mod_filenames.push(dir_name);
        }

        Self::generate_mod_rs(&out_dir, &mod_filenames)?;

        Ok(mod_filenames)
    }

    fn generate_simfs(out_dir: impl AsRef<Path>, simfs: &[impl AsRef<Path>]) -> Result<Vec<String>, BuildError> {
        let mut module_files = Vec::with_capacity(simfs.len());

        for simf_file_path in simfs {
            let mod_name = Self::generate_simf_file(out_dir.as_ref(), simf_file_path)?;
            module_files.push(mod_name);
        }

        Ok(module_files)
    }

    fn generate_simf_file(out_dir: impl AsRef<Path>, simf_file_path: impl AsRef<Path>) -> Result<String, BuildError> {
        let simf_file_buf = PathBuf::from(simf_file_path.as_ref());
        let simf_content =
            SimfContent::extract_content_from_path(&simf_file_buf).map_err(BuildError::FailedToExtractContent)?;

        let contract_name = simf_content.contract_name.clone();
        let output_file = out_dir.as_ref().join(format!("{}.rs", &contract_name));

        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&output_file)?;
        let code = Self::generate_simf_binding_code(simf_content, simf_file_buf)?;

        Self::expand_file(code, &mut file)?;

        Ok(contract_name)
    }

    fn generate_mod_rs(out_dir: impl AsRef<Path>, simfs_mod_names: &[String]) -> Result<(), BuildError> {
        let output_file = out_dir.as_ref().join("mod.rs");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&output_file)?;
        let code = Self::generate_mod_binding_code(simfs_mod_names)?;

        Self::expand_file(code, &mut file)?;

        Ok(())
    }

    fn expand_file(code: TokenStream, buf: &mut dyn Write) -> Result<(), BuildError> {
        let file: syn::File = syn::parse2(code).map_err(|e| BuildError::GenerationFailed(e.to_string()))?;
        let prettystr = prettyplease::unparse(&file);

        buf.write_all(prettystr.as_bytes())?;
        buf.flush()?;

        Ok(())
    }

    fn generate_simf_binding_code(simf_content: SimfContent, simf_file: PathBuf) -> Result<TokenStream, BuildError> {
        let cwd = env::current_dir()?;
        let contract_name = &simf_content.contract_name;
        let program_name = {
            let base_name = convert_contract_name_to_struct_name(contract_name);
            format_ident!("{base_name}Program")
        };
        let include_simf_source_const = convert_contract_name_to_contract_source_const(contract_name);
        let include_simf_module = convert_contract_name_to_contract_module(contract_name);

        let pathdiff = pathdiff::diff_paths(&simf_file, &cwd).ok_or(BuildError::FailedToFindCorrectRelativePath {
            cwd: cwd,
            simf_file: simf_file,
        })?;
        let pathdiff = pathdiff.to_string_lossy().into_owned();

        let code = quote! {
            use simplex::include_simf;
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

    fn generate_mod_binding_code(mod_names: &[String]) -> Result<TokenStream, BuildError> {
        let mod_names = mod_names.iter().map(|x| format_ident!("{x}")).collect::<Vec<_>>();

        let code = quote! {
            #(pub mod #mod_names);*;
        };

        Ok(code)
    }
}
