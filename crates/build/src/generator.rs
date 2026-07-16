use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use globwalk::FileType;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use simplicityhl::TemplateProgram;
use simplicityhl::UnstableFeatures;
use simplicityhl::resolution::DependencyMap;
use simplicityhl::resolution::ValidatedDeps;
use simplicityhl::source::CanonPath;
use simplicityhl::source::CanonSourceFile;

use crate::deps::DepSources;
use crate::macros::codegen::convert_contract_name_to_struct_name;
use crate::macros::parse::SimfContent;

use super::error::BuildError;

/// The generated module every binding's `SOURCES` const points into.
const SOURCES_MODULE: &str = "project_sources";

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
    /// The contract's ABI as JSON, extracted once so the runtime needs no compiler.
    abi_json: String,
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
        deps: &DepSources,
        compiler_version: &str,
        simc: &Path,
    ) -> Result<(), BuildError> {
        let cwd = env::current_dir()?;
        let out_dir = out_dir.as_ref();
        let base_dir = base_dir.as_ref();

        let pathdiff = pathdiff::diff_paths(base_dir, &cwd).ok_or(BuildError::FailedToFindCorrectRelativePath {
            cwd,
            simf_file: base_dir.to_path_buf(),
        })?;

        let simf_out_dir = out_dir.join(pathdiff);

        // Original (un-flattened) sources the runtime materializes so the pinned
        // `simc` can resolve each contract's imports off disk.
        let mut source_set = Self::gather_sources(base_dir)?;
        source_set.extend(deps.files.iter().map(|f| (f.embed_path.clone(), f.contents.clone())));

        // The linked frontend version-checks `simc` directives against its own
        // version, so flatten and ABI typing run on a directive-stripped temp mirror.
        let mirror = Self::materialize_stripped(&source_set)?;
        let stripped_root = mirror.path();

        let mirror_deps = deps.validated_in(stripped_root)?;
        let dep_flags = deps.dep_flags(stripped_root);

        let artifacts = simfs
            .iter()
            .map(|s| {
                Self::process_simf(
                    s.as_ref(),
                    base_dir,
                    &mirror_deps,
                    &dep_flags,
                    &simf_out_dir,
                    stripped_root,
                    simc,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let tree = Self::build_tree(artifacts)?;

        Self::generate_bindings(out_dir, tree, compiler_version, &source_set, &deps.triples)?;

        Ok(())
    }

    /// Directive-stripped copies of `source_set` in a fresh temp dir (removed on
    /// drop), so the linked frontend never sees a `simc` directive.
    fn materialize_stripped(source_set: &[(String, String)]) -> Result<tempfile::TempDir, BuildError> {
        let root = tempfile::Builder::new().prefix("smplx-build-").tempdir()?;
        for (relative, content) in source_set {
            let path = root.path().join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, crate::version::without_directive(content).as_bytes())?;
        }
        Ok(root)
    }

    /// Every `.simf` under `base_dir` as `(relative-path, contents)`, forward-slashed
    /// for a platform-stable embedded layout.
    fn gather_sources(base_dir: &Path) -> Result<Vec<(String, String)>, BuildError> {
        let walker = globwalk::GlobWalkerBuilder::from_patterns(base_dir, &["**/*.simf"])
            .follow_links(true)
            .file_type(FileType::FILE)
            .build()?
            .filter_map(Result::ok);

        let mut sources = Vec::new();
        for entry in walker {
            let path = entry.path();
            let relative = path
                .strip_prefix(base_dir)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            sources.push((relative, fs::read_to_string(path)?));
        }
        Ok(sources)
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

    /// Processes one `.simf` into a [`SimfArtifact`]: writes its flattened source and
    /// ABI sidecar under `simf_out_dir` and captures its contract name.
    fn process_simf(
        source: &Path,
        base_dir: &Path,
        validated_deps: &ValidatedDeps,
        dep_flags: &[String],
        simf_out_dir: &Path,
        stripped_root: &Path,
        simc: &Path,
    ) -> Result<SimfArtifact, BuildError> {
        let relative_path = source
            .strip_prefix(base_dir)
            .map_err(BuildError::NoBasePathForGeneration)?
            .to_path_buf();

        let mirrored_path = simf_out_dir.join(&relative_path);

        if let Some(parent) = mirrored_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = Self::process_content(&relative_path, validated_deps, stripped_root)?;
        fs::write(&mirrored_path, &content)?;

        // Written beside the mirrored source, so the macro and runtime read it
        // instead of re-invoking a compiler.
        let abi_json = Self::extract_abi_via_simc(simc, &stripped_root.join(&relative_path), dep_flags)?;
        let sidecar = PathBuf::from(format!("{}.abi.json", mirrored_path.display()));
        fs::write(&sidecar, &abi_json)?;

        let contract_name = SimfContent::extract_content_from_path(&source.to_path_buf())
            .map_err(BuildError::FailedToExtractContent)?
            .contract_name;

        Ok(SimfArtifact {
            relative_path,
            mirrored_path,
            contract_name,
            abi_json,
        })
    }

    /// Extracts the ABI JSON with the pinned `simc --abi-only`, run against the
    /// stripped entry so imports resolve off disk.
    fn extract_abi_via_simc(simc: &Path, stripped_entry: &Path, dep_flags: &[String]) -> Result<String, BuildError> {
        let mut cmd = Command::new(simc);
        cmd.arg(stripped_entry)
            .arg("--abi-only")
            .arg("-Z")
            .arg("imports")
            .arg("--json");
        for flag in dep_flags {
            cmd.arg("--dep").arg(flag);
        }
        let output = cmd
            .output()
            .map_err(|e| BuildError::GenerationFailed(format!("running {}: {e}", simc.display())))?;
        if !output.status.success() {
            return Err(BuildError::GenerationFailed(format!(
                "simc --abi-only failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| BuildError::GenerationFailed(format!("parsing simc --abi-only output: {e}")))?;
        let abi_meta = value
            .get("abi_meta")
            .ok_or_else(|| BuildError::GenerationFailed("simc --abi-only output has no abi_meta".to_string()))?;
        serde_json::to_string(abi_meta)
            .map_err(|e| BuildError::GenerationFailed(format!("re-serializing abi_meta: {e}")))
    }

    /// Flattens one contract into a self-contained source, reading from the
    /// directive-stripped mirror.
    fn process_content(
        relative: &Path,
        validated_deps: &ValidatedDeps,
        stripped_root: &Path,
    ) -> Result<String, BuildError> {
        let source = stripped_root.join(relative);
        let parent_dir = source.parent().ok_or_else(|| {
            BuildError::GenerationFailed(format!("Path '{}' has no parent directory", source.display()))
        })?;

        let canon_source = CanonPath::canonicalize(&source).map_err(BuildError::PathCanonicalization)?;
        let content = fs::read_to_string(&source)?;
        let canon_source_file = CanonSourceFile::new(canon_source, Arc::from(content));
        let dependency_map = Self::build_dependency_map(validated_deps, parent_dir)?;

        TemplateProgram::flatten(canon_source_file, &dependency_map, &UnstableFeatures::all())
            .map_err(|errors| BuildError::Flattening(errors.to_string()))
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

    /// Writes the shared sources module at the root, then one binding per contract.
    fn generate_bindings(
        out_dir: &Path,
        tree: TreeNode,
        compiler_version: &str,
        source_set: &[(String, String)],
        dep_triples: &[(String, String, String)],
    ) -> Result<(), BuildError> {
        fs::create_dir_all(out_dir)?;
        Self::generate_project_sources(out_dir, source_set, dep_triples)?;
        Self::generate_bindings_level(out_dir, tree, compiler_version, 0, vec![SOURCES_MODULE.to_string()])
    }

    /// Recursively generates bindings for one directory level; `depth` counts the
    /// directories below the root, where the shared sources module lives.
    fn generate_bindings_level(
        out_dir: &Path,
        tree: TreeNode,
        compiler_version: &str,
        depth: usize,
        mut mod_names: Vec<String>,
    ) -> Result<(), BuildError> {
        fs::create_dir_all(out_dir)?;

        for artifact in tree.files {
            let mod_name = Self::generate_simf_binding(out_dir, artifact, compiler_version, depth)?;
            mod_names.push(mod_name);
        }

        for (dir_name, subtree) in tree.dirs {
            Self::generate_bindings_level(
                &out_dir.join(&dir_name),
                subtree,
                compiler_version,
                depth + 1,
                Vec::new(),
            )?;
            mod_names.push(dir_name);
        }

        Self::generate_mod_rs(out_dir, &mod_names)?;

        Ok(())
    }

    /// The shared sources module: the original sources embedded once, plus the
    /// remappings that resolve `use <alias>::…` imports among them.
    fn generate_project_sources(
        out_dir: &Path,
        source_set: &[(String, String)],
        dep_triples: &[(String, String, String)],
    ) -> Result<(), BuildError> {
        let entries = source_set.iter().map(|(path, contents)| {
            quote! { (#path, #contents) }
        });
        let deps = dep_triples.iter().map(|(context, alias, target)| {
            quote! { (#context, #alias, #target) }
        });
        let code = quote! {
            pub const PROJECT_SOURCES: &[(&str, &str)] = &[
                #(#entries),*
            ];
            pub const PROJECT_DEPS: &[(&str, &str, &str)] = &[
                #(#deps),*
            ];
        };

        let output_file = out_dir.join(format!("{SOURCES_MODULE}.rs"));
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&output_file)?;
        Self::expand_file(code, &mut file)
    }

    /// Generates a single `.rs` binding file for one simf artifact.
    fn generate_simf_binding(
        out_dir: &Path,
        artifact: SimfArtifact,
        compiler_version: &str,
        depth: usize,
    ) -> Result<String, BuildError> {
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

        // The file to compile, forward-slashed to match the embedded source set.
        let entry = artifact.relative_path.to_string_lossy().replace('\\', "/");

        let code = Self::generate_simf_binding_code(
            &artifact.contract_name,
            &pathdiff,
            compiler_version,
            &entry,
            depth,
            &artifact.abi_json,
        )?;

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

    fn generate_simf_binding_code(
        contract_name: &str,
        target_simf: &Path,
        compiler_version: &str,
        entry: &str,
        depth: usize,
        abi_json: &str,
    ) -> Result<TokenStream, BuildError> {
        let program_name = {
            let base_name = convert_contract_name_to_struct_name(contract_name);
            format_ident!("{base_name}Program")
        };

        let target_simf_str = target_simf.to_string_lossy().into_owned();

        // One `super` to leave the binding module, plus one per directory of nesting.
        let sources_module = format_ident!("{SOURCES_MODULE}");
        let supers: Vec<_> = std::iter::repeat_n(quote! { super:: }, depth + 1).collect();
        let project_sources = quote! { #(#supers)* #sources_module::PROJECT_SOURCES };
        let project_deps = quote! { #(#supers)* #sources_module::PROJECT_DEPS };

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
                pub const COMPILER_VERSION: &'static str = #compiler_version;
                pub const ENTRY: &'static str = #entry;
                pub const SOURCES: &'static [(&'static str, &'static str)] = #project_sources;
                pub const DEPS: &'static [(&'static str, &'static str, &'static str)] = #project_deps;
                pub const ABI: &'static str = #abi_json;

                #[must_use]
                pub fn new(arguments: impl ArgumentsTrait + 'static) -> Self {
                    Self {
                        program: Program::from_sources(Self::SOURCES, Self::ENTRY, Box::new(arguments))
                            .with_deps(Self::DEPS)
                            .with_compiler_version(Self::COMPILER_VERSION)
                            .with_abi_json(Self::ABI),
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
