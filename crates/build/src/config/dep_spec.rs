use std::fmt::Write;

use crate::error::TomlEditError;

pub(super) struct DepSpec {
    pub alias: String,
    pub source: Source,
}

pub(super) enum Source {
    Git(String),
    Path(String),
}

impl DepSpec {
    /// Parses a raw CLI token into a [`DepSpec`].
    ///
    /// Accepted forms:
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
    pub(super) fn parse_dep(raw: &str) -> Result<DepSpec, TomlEditError> {
        let (alias, source_str) = match raw.split_once('=') {
            Some((a, s)) if !a.is_empty() && !s.is_empty() => (a.to_owned(), s),
            Some(_) => return Err(TomlEditError::MalformedDep(raw.to_owned())),
            None => (Self::derive_alias(raw)?, raw),
        };

        let source = Self::classify_source(source_str);

        Ok(DepSpec { alias, source })
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

            let source = match &spec.source {
                Source::Git(url) => url.as_str(),
                Source::Path(p) => p.as_str(),
            };
            let _ = write!(out, "\n    {} = {}", spec.alias, source);
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
            Source::Git(s.to_owned())
        } else {
            Source::Path(s.to_owned())
        }
    }
}
