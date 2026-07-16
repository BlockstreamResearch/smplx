//! The dependency graph in relocatable form: each dependency root becomes a
//! `__deps__/<n>` slot, so the same embedded sources and `(context, alias, target)`
//! remappings serve the build, the generated bindings, and the runtime scratch
//! directory handed to `simc --dep`.

use std::path::{Path, PathBuf};

use globwalk::FileType;
use simplicityhl::resolution::{DependencyMapBuilder, ValidatedDeps};
use simplicityhl::source::CanonPath;

use crate::error::BuildError;

/// Embedded dependency roots live here; not a valid SimplicityHL identifier, so it
/// cannot collide with a `use` path.
const SLOT_PREFIX: &str = "__deps__";

/// One dependency `.simf` file.
pub struct DepFile {
    /// Path within the embedded source set (`__deps__/<n>/…`), forward-slashed.
    pub embed_path: String,
    /// The real on-disk file, for directive conflict reporting.
    pub real_path: PathBuf,
    pub contents: String,
}

/// The project's dependency graph, detached from its on-disk locations.
#[derive(Default)]
pub struct DepSources {
    /// `(context, alias, target)` as embed-relative paths (`""` is the source
    /// root), sorted for reproducible bindings.
    pub triples: Vec<(String, String, String)>,
    pub files: Vec<DepFile>,
}

impl DepSources {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.triples.is_empty()
    }

    /// Detaches the collector's raw remappings from their on-disk roots into
    /// reproducible slots; aliases of one directory share a slot.
    pub(crate) fn from_raw(
        source_root: &CanonPath,
        raw: &[(CanonPath, String, CanonPath)],
    ) -> Result<Self, BuildError> {
        let mut roots: Vec<&CanonPath> = raw.iter().map(|(_, _, target)| target).collect();
        roots.sort();
        roots.dedup();
        let slot_of = |dir: &CanonPath| -> Option<String> {
            if dir == source_root {
                return Some(String::new());
            }
            let index = roots.binary_search(&dir).ok()?;
            Some(format!("{SLOT_PREFIX}/{index}"))
        };

        let mut triples = Vec::with_capacity(raw.len());
        for (context, alias, target) in raw {
            let context = slot_of(context).ok_or_else(|| {
                BuildError::DependencyMap(format!(
                    "dependency context {} is not in the graph",
                    context.as_path().display()
                ))
            })?;
            let target = slot_of(target).expect("every target was assigned a slot");
            triples.push((context, alias.clone(), target));
        }
        triples.sort();

        let mut files = Vec::new();
        for (index, root) in roots.iter().enumerate() {
            files.extend(Self::read_root(root.as_path(), &format!("{SLOT_PREFIX}/{index}"))?);
        }
        files.sort_by(|a, b| a.embed_path.cmp(&b.embed_path));

        Ok(Self { triples, files })
    }

    /// Every `.simf` under one dependency root, as [`DepFile`]s in `slot`.
    fn read_root(root: &Path, slot: &str) -> Result<Vec<DepFile>, BuildError> {
        let walker = globwalk::GlobWalkerBuilder::from_patterns(root, &["**/*.simf"])
            .follow_links(true)
            .file_type(FileType::FILE)
            .build()?
            .filter_map(Result::ok);

        let mut files = Vec::new();
        for entry in walker {
            let real_path = entry.path().to_path_buf();
            let relative = real_path
                .strip_prefix(root)
                .unwrap_or(&real_path)
                .to_string_lossy()
                .replace('\\', "/");
            files.push(DepFile {
                embed_path: format!("{slot}/{relative}"),
                real_path: real_path.clone(),
                contents: std::fs::read_to_string(&real_path)?,
            });
        }
        Ok(files)
    }

    /// The graph re-rooted at `root` (a directory the embedded layout was
    /// materialized into), validated for the linked frontend's flatten.
    pub fn validated_in(&self, root: &Path) -> Result<ValidatedDeps, BuildError> {
        let mut builder = DependencyMapBuilder::new();
        for (context, alias, target) in &self.triples {
            // A dir holding no `.simf` files is never created by the file writes;
            // ensure it exists so canonicalization cannot fail.
            std::fs::create_dir_all(root.join(target))?;
            std::fs::create_dir_all(root.join(context))?;
            builder.add_dependency(
                Self::canon(&root.join(context))?,
                alias.clone(),
                Self::canon(&root.join(target))?,
            );
        }
        builder
            .validate_deps()
            .map_err(|e| BuildError::DependencyMap(e.to_string()))
    }

    /// `--dep CONTEXT:ALIAS=TARGET` values for a `simc` run materialized under `root`.
    #[must_use]
    pub fn dep_flags(&self, root: &Path) -> Vec<String> {
        self.triples
            .iter()
            .map(|(context, alias, target)| {
                format!(
                    "{}:{alias}={}",
                    root.join(context).display(),
                    root.join(target).display()
                )
            })
            .collect()
    }

    fn canon(path: &Path) -> Result<CanonPath, BuildError> {
        CanonPath::canonicalize(path).map_err(BuildError::PathCanonicalization)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A project source root plus two dependency roots forming a diamond (`b` used
    /// by both the project and `a`).
    fn layout(name: &str) -> (PathBuf, CanonPath, CanonPath, CanonPath) {
        let root = std::env::temp_dir().join(format!("smplx-deps-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["src", "a/simf", "b/simf/nested"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("a/simf/lib_a.simf"), "pub fn a() {}").unwrap();
        std::fs::write(root.join("b/simf/lib_b.simf"), "pub fn b() {}").unwrap();
        std::fs::write(root.join("b/simf/nested/extra.simf"), "pub fn c() {}").unwrap();

        let canon = |p: &str| CanonPath::canonicalize(&root.join(p)).unwrap();
        let (src, a, b) = (canon("src"), canon("a/simf"), canon("b/simf"));
        (root, src, a, b)
    }

    #[test]
    fn detaches_diamond_graph_into_slots() {
        let (root, src, a, b) = layout("diamond");
        let raw = vec![
            (src.clone(), "a".to_string(), a.clone()),
            (a.clone(), "b".to_string(), b.clone()),
            (src.clone(), "b".to_string(), b.clone()),
        ];
        let deps = DepSources::from_raw(&src, &raw).unwrap();

        // Slots sort by path: a/simf -> 0, b/simf -> 1; the transitive remapping's
        // context is a's own slot.
        assert_eq!(
            deps.triples,
            vec![
                (String::new(), "a".to_string(), "__deps__/0".to_string()),
                (String::new(), "b".to_string(), "__deps__/1".to_string()),
                ("__deps__/0".to_string(), "b".to_string(), "__deps__/1".to_string()),
            ]
        );

        let embed: Vec<&str> = deps.files.iter().map(|f| f.embed_path.as_str()).collect();
        assert_eq!(
            embed,
            [
                "__deps__/0/lib_a.simf",
                "__deps__/1/lib_b.simf",
                "__deps__/1/nested/extra.simf"
            ]
        );
        assert_eq!(deps.files[0].contents, "pub fn a() {}");

        let flags = deps.dep_flags(Path::new("/scratch"));
        assert_eq!(flags[0], "/scratch/:a=/scratch/__deps__/0");
        assert_eq!(flags[2], "/scratch/__deps__/0:b=/scratch/__deps__/1");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_unknown_context() {
        let (root, src, a, _) = layout("unknown-ctx");
        // `a` never appears as a target, so as a context it is outside the graph.
        let raw = vec![(a, "x".to_string(), src.clone())];
        assert!(DepSources::from_raw(&src, &raw).is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
