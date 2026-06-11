use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use simplicityhl::TemplateProgram;
use simplicityhl::resolution::DependencyMap;
use simplicityhl::resolution::ValidatedDeps;
use simplicityhl::source::CanonPath;
use simplicityhl::source::CanonSourceFile;

use crate::macros::codegen::{
    convert_contract_name_to_contract_module, convert_contract_name_to_contract_source_const,
    convert_contract_name_to_struct_name,
};
use crate::macros::parse::SimfContent;

use super::error::BuildError;

pub struct ArtifactsGenerator {}

/// A single processed `.simf` file with all metadata needed for binding generation.
///
/// Created once per source file and carries everything downstream — no recomputation
/// of paths or contract names in later stages.
struct SimfArtifact {
    /// Path relative to `base_dir` (e.g. `hash/func/sha256.simf`).
    /// Used to mirror the source structure under `out_dir/simf/`.
    relative_path: PathBuf,
    /// Full path to the file written under `out_dir/simf/`.
    /// Passed directly to `include_simf!` — no path reconstruction needed.
    mirrored_path: PathBuf,
    /// Contract name extracted from the `.simf` source file.
    contract_name: String,
}

#[derive(Default)]
struct TreeNode {
    files: Vec<SimfArtifact>,
    dirs: HashMap<String, TreeNode>,
}

impl ArtifactsGenerator {
    pub fn generate_artifacts(
        out_dir: impl AsRef<Path>,
        base_dir: impl AsRef<Path>,
        simfs: &[impl AsRef<Path>],
        validated_deps: &ValidatedDeps,
    ) -> Result<(), BuildError> {
        let cwd = env::current_dir()?;
        let out_dir = out_dir.as_ref();
        let base_dir = base_dir.as_ref();

        let pathdiff = pathdiff::diff_paths(base_dir, &cwd).ok_or(BuildError::FailedToFindCorrectRelativePath {
            cwd,
            simf_file: base_dir.to_path_buf(),
        })?;

        let simf_out_dir = out_dir.join(pathdiff);

        let artifacts = simfs
            .iter()
            .map(|s| Self::process_simf(s.as_ref(), base_dir, validated_deps, &simf_out_dir))
            .collect::<Result<Vec<_>, _>>()?;

        let tree = Self::build_tree(artifacts)?;
        Self::generate_bindings(out_dir, tree)?;

        Ok(())
    }

    pub fn build_dependency_map(
        validated_deps: &ValidatedDeps,
        entry_root_dir: impl AsRef<Path>,
    ) -> Result<DependencyMap, BuildError> {
        let canon_entry_root =
            CanonPath::canonicalize(entry_root_dir.as_ref()).map_err(BuildError::PathCanonicalization)?;

        validated_deps
            .with_root(canon_entry_root)
            .map_err(|e| BuildError::DependencyMap(e.to_string()))
    }

    /// Processes a single `.simf` source file:
    /// - Writes its content to the mirrored path under `simf_out_dir`
    /// - Extracts the contract name
    ///
    /// All path and name information needed for downstream stages is captured
    /// here so later steps never need to re-derive anything.
    fn process_simf(
        source: &Path,
        base_dir: &Path,
        validated_deps: &ValidatedDeps,
        simf_out_dir: &Path,
    ) -> Result<SimfArtifact, BuildError> {
        let relative_path = source
            .strip_prefix(base_dir)
            .map_err(BuildError::NoBasePathForGeneration)?
            .to_path_buf();

        let mirrored_path = simf_out_dir.join(&relative_path);

        if let Some(parent) = mirrored_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = Self::process_content(source, validated_deps)?;
        fs::write(&mirrored_path, &content)?;

        let contract_name = SimfContent::extract_content_from_path(&source.to_path_buf())
            .map_err(BuildError::FailedToExtractContent)?
            .contract_name;

        Ok(SimfArtifact {
            relative_path,
            mirrored_path,
            contract_name,
        })
    }

    /// Reads and processes the content of a `.simf` file.
    fn process_content(source: &Path, validated_deps: &ValidatedDeps) -> Result<String, BuildError> {
        let parent_dir = source.parent().ok_or_else(|| {
            BuildError::GenerationFailed(format!("Path '{}' has no parent directory", source.display()))
        })?;

        let canon_source = CanonPath::canonicalize(source).map_err(BuildError::PathCanonicalization)?;
        let content = fs::read_to_string(source)?;
        let canon_source_file = CanonSourceFile::new(canon_source, Arc::from(content));
        let dependency_map = Self::build_dependency_map(validated_deps, parent_dir)?;

        TemplateProgram::flatten(canon_source_file, &dependency_map).map_err(BuildError::Flattening)
    }

    /// Arranges a flat list of artifacts into a tree mirroring the source directory layout.
    fn build_tree(artifacts: Vec<SimfArtifact>) -> Result<TreeNode, BuildError> {
        let mut root = TreeNode::default();

        for artifact in artifacts {
            let components: Vec<_> = artifact
                .relative_path
                .components()
                .filter_map(|c| {
                    if let Component::Normal(name) = c {
                        Some(name.to_string_lossy().into_owned())
                    } else {
                        None
                    }
                })
                .collect();

            // All components except the last are directories; the last is the file itself
            let mut current = &mut root;
            for dir in &components[..components.len().saturating_sub(1)] {
                current = current.dirs.entry(dir.clone()).or_default();
            }
            current.files.push(artifact);
        }

        Ok(root)
    }

    /// Recursively generates bindings for every node in the tree.
    fn generate_bindings(out_dir: &Path, tree: TreeNode) -> Result<(), BuildError> {
        fs::create_dir_all(out_dir)?;

        let mut mod_names = Vec::new();

        for artifact in tree.files {
            let mod_name = Self::generate_simf_binding(out_dir, artifact)?;
            mod_names.push(mod_name);
        }

        for (dir_name, subtree) in tree.dirs {
            Self::generate_bindings(&out_dir.join(&dir_name), subtree)?;
            mod_names.push(dir_name);
        }

        Self::generate_mod_rs(out_dir, &mod_names)?;

        Ok(())
    }

    /// Generates a single `.rs` binding file for one simf artifact.
    fn generate_simf_binding(out_dir: &Path, artifact: SimfArtifact) -> Result<String, BuildError> {
        let output_file = out_dir.join(format!("{}.rs", &artifact.contract_name));

        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&output_file)?;

        let cwd = env::current_dir()?;
        let pathdiff =
            pathdiff::diff_paths(&artifact.mirrored_path, &cwd).ok_or(BuildError::FailedToFindCorrectRelativePath {
                cwd,
                simf_file: artifact.mirrored_path.clone(),
            })?;

        let code = Self::generate_simf_binding_code(&artifact.contract_name, &pathdiff)?;
        Self::expand_file(code, &mut file)?;

        Ok(artifact.contract_name)
    }

    fn generate_mod_rs(out_dir: impl AsRef<Path>, mod_names: &[String]) -> Result<(), BuildError> {
        let output_file = out_dir.as_ref().join("mod.rs");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&output_file)?;

        let code = Self::generate_mod_binding_code(mod_names)?;
        Self::expand_file(code, &mut file)?;

        Ok(())
    }

    fn expand_file(code: TokenStream, buf: &mut dyn Write) -> Result<(), BuildError> {
        let file: syn::File = syn::parse2(code).map_err(|e| BuildError::GenerationFailed(e.to_string()))?;
        let prettystr = prettyplease::unparse(&file);

        buf.write_all(b"// This file is @generated by Simplex. Do not edit manually.\n\n")?;
        buf.write_all(prettystr.as_bytes())?;
        buf.flush()?;

        Ok(())
    }

    fn generate_simf_binding_code(contract_name: &str, target_simf: &Path) -> Result<TokenStream, BuildError> {
        let program_name = {
            let base_name = convert_contract_name_to_struct_name(contract_name);
            format_ident!("{base_name}Program")
        };

        let include_simf_source_const = convert_contract_name_to_contract_source_const(contract_name);
        let include_simf_module = convert_contract_name_to_contract_module(contract_name);
        let target_simf_str = target_simf.to_string_lossy().into_owned();

        let code = quote! {
            use simplex::include_simf;
            use simplex::program::{ArgumentsTrait, Program};
            use simplex::provider::SimplicityNetwork;
            use simplex::simplicityhl::elements::Script;
            use simplex::simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey;

            pub struct #program_name {
                program: Program,
            }

            impl #program_name {
                pub const SOURCE: &'static str = #include_simf_module::#include_simf_source_const;

                #[must_use]
                pub fn new(arguments: impl ArgumentsTrait + 'static) -> Self {
                    Self {
                        program: Program::new(Self::SOURCE, Box::new(arguments)),
                    }
                }

                #[must_use]
                pub fn with_taproot_pubkey(mut self, pub_key: XOnlyPublicKey) -> Self {
                    self.program = self.program.with_taproot_pubkey(pub_key);
                    self
                }

                #[must_use]
                pub fn with_storage_capacity(mut self, capacity: usize) -> Self {
                    self.program = self.program.with_storage_capacity(capacity);
                    self
                }

                #[must_use]
                pub fn set_storage_at(&mut self, index: usize, new_value: [u8; 32]) {
                    self.program.set_storage_at(index, new_value);
                }

                #[must_use]
                pub fn get_storage_len(&self) -> usize {
                    self.program.get_storage_len()
                }

                #[must_use]
                pub fn get_storage(&self) -> &[[u8; 32]] {
                    self.program.get_storage()
                }

                #[must_use]
                pub fn get_storage_at(&self, index: usize) -> [u8; 32] {
                    self.program.get_storage_at(index)
                }

                #[must_use]
                pub fn get_script_pubkey(&self, network: &SimplicityNetwork) -> Script {
                    self.program.get_script_pubkey(network)
                }

                #[must_use]
                pub fn get_script_hash(&self, network: &SimplicityNetwork) -> [u8; 32] {
                    self.program.get_script_hash(network)
                }
            }

            impl AsRef<Program> for #program_name {
                fn as_ref(&self) -> &Program {
                    &self.program
                }
            }

            impl AsMut<Program> for #program_name {
                fn as_mut(&mut self) -> &mut Program {
                    &mut self.program
                }
            }

            include_simf!(#target_simf_str);
        };

        Ok(code)
    }

    fn generate_mod_binding_code(mod_names: &[String]) -> Result<TokenStream, BuildError> {
        let mod_idents = mod_names.iter().map(|x| format_ident!("{x}")).collect::<Vec<_>>();

        let code = quote! {
            #![allow(clippy::all)]
            #(
                #[rustfmt::skip]
                pub mod #mod_idents;
            )*
        };

        Ok(code)
    }
}
