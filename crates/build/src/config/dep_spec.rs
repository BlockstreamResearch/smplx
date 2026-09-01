use std::fmt::Write;
use std::process::Command;

use toml_edit::{InlineTable, Value};

use crate::error::TomlEditError;

/// The bare token that resolves to the `SimplicityHL` standard library.
const STD_ALIAS: &str = "std";

/// The repository backing [`STD_ALIAS`].
const STD_URL: &str = "https://github.com/BlockstreamResearch/simplicityhl-std.git";

pub(super) struct DepSpec {
    pub alias: String,
    pub source: Source,
}

pub(super) enum Source {
    Git { url: String, tag: Option<String> },
    Path(String),
}

impl DepSpec {
    /// Parses a raw CLI token into a [`DepSpec`].
    ///
    /// Accepted forms:
    /// - `std`: Shorthand for [`STD_URL`], pinned to its latest release tag.
    /// - `<source>`: The alias is derived from the last path segment of the source,
    ///   with a trailing `.git` stripped.
    /// - `<alias>=<source>`: Both parts must be non-empty.
    ///
    /// The source is then classified as [`Source::Git`] or [`Source::Path`] based on
    /// its scheme or `.git` suffix.
    ///
    /// # Errors
    /// - `TomlEditError::MalformedDep`: If `raw` contains `=` but either side is empty,
    ///   or if the alias cannot be derived from the source (e.g. the source contains
    ///   no non-empty path segment).
    /// - `TomlEditError::RemoteTags` / `TomlEditError::NoTags`: If `raw` is `std` and
    ///   its latest tag cannot be resolved.
    pub(super) fn parse_dep(raw: &str) -> Result<DepSpec, TomlEditError> {
        if raw == STD_ALIAS {
            return Self::std_spec();
        }

        let (alias, source_str) = match raw.split_once('=') {
            Some((a, s)) if !a.is_empty() && !s.is_empty() => (a.to_owned(), s),
            Some(_) => return Err(TomlEditError::MalformedDep(raw.to_owned())),
            None => (Self::derive_alias(raw)?, raw),
        };

        let source = Self::classify_source(source_str);

        Ok(DepSpec { alias, source })
    }

    /// Builds the spec for the standard library, pinned to the newest tag currently
    /// published by [`STD_URL`].
    fn std_spec() -> Result<DepSpec, TomlEditError> {
        let tag = Self::latest_tag(STD_URL)?;

        Ok(DepSpec {
            alias: STD_ALIAS.to_owned(),
            source: Source::Git {
                url: STD_URL.to_owned(),
                tag: Some(tag),
            },
        })
    }

    /// Returns the highest release tag advertised by the remote at `url`.
    ///
    /// # Errors
    /// - `TomlEditError::RemoteTags`: If `git ls-remote` cannot be run, exits non-zero,
    ///   or emits non-UTF-8 output.
    /// - `TomlEditError::NoTags`: If the remote advertises no release tag.
    fn latest_tag(url: &str) -> Result<String, TomlEditError> {
        let failed = |reason: String| TomlEditError::RemoteTags {
            url: url.to_owned(),
            reason,
        };

        let output = Command::new("git")
            .args(["ls-remote", "--tags", "--refs", "--sort=-v:refname", url])
            .output()
            .map_err(|err| failed(err.to_string()))?;

        if !output.status.success() {
            return Err(failed(String::from_utf8_lossy(&output.stderr).trim().to_owned()));
        }

        let stdout = String::from_utf8(output.stdout).map_err(|err| failed(err.to_string()))?;

        stdout
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .filter_map(|(_, reference)| reference.strip_prefix("refs/tags/"))
            // skip pre-releases
            .find(|tag| !tag.contains('-'))
            .map(str::to_owned)
            .ok_or_else(|| TomlEditError::NoTags(url.to_owned()))
    }

    /// Renders the spec as the inline table written under `[dependencies]`.
    #[must_use]
    pub(super) fn to_inline(&self) -> InlineTable {
        let mut inline = InlineTable::new();

        match &self.source {
            Source::Git { url, tag } => {
                inline.insert("git", Value::from(url.as_str()));

                if let Some(tag) = tag {
                    inline.insert("tag", Value::from(tag.as_str()));
                }
            }
            Source::Path(p) => {
                inline.insert("path", Value::from(p.as_str()));
            }
        }

        inline
    }

    /// Formats a batch of dependency specs as a bracketed, one-per-line list.
    #[must_use]
    pub(super) fn format_batch(specs: &[DepSpec]) -> String {
        if specs.is_empty() {
            return "[]".to_owned();
        }

        let mut out = String::from("[");

        for (index, spec) in specs.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }

            let _ = write!(out, "\n    {} = {}", spec.alias, spec.to_inline());
        }

        out.push_str("\n]");

        out
    }

    /// Derives a default alias from a source string by taking its last non-empty
    /// path segment and stripping a trailing `.git`.
    ///
    /// # Errors
    /// - `BuildError::MalformedDep`: If `source` contains no non-empty path segment
    ///   (e.g. an empty string or one consisting only of separators).
    fn derive_alias(source: &str) -> Result<String, TomlEditError> {
        let last = source
            .rsplit(['/', '\\'])
            .find(|s| !s.is_empty())
            .ok_or_else(|| TomlEditError::MalformedDep(source.to_owned()))?;

        Ok(last.trim_end_matches(".git").to_owned())
    }

    /// Classifies a source string as [`Source::Git`] if it carries a recognised
    /// scheme (`http`, `https`, `git`, `ssh`) or ends in `.git`, otherwise as
    /// [`Source::Path`]. The `.git` check is case-insensitive.
    fn classify_source(s: &str) -> Source {
        let git_ext = std::path::Path::new(s)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("git"));

        if s.starts_with("http://")
            || s.starts_with("https://")
            || s.starts_with("git://")
            || s.starts_with("ssh://")
            || git_ext
        {
            Source::Git {
                url: s.to_owned(),
                tag: None,
            }
        } else {
            Source::Path(s.to_owned())
        }
    }
}
