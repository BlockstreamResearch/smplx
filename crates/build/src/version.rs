//! Reads the `simc "<range>";` directives `.simf` files declare and picks the
//! newest compiler release that satisfies all of them.

use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};

use globwalk::FileType;
pub use semver::Version;
use simplicityhl::version::{SimcDirective, VersionRequirement};

use crate::error::BuildError;

/// A version requirement declared by one `.simf` file.
#[derive(Clone, Debug)]
pub struct FileRequirement {
    /// The declaring file, for conflict reporting.
    pub path: PathBuf,
    pub requirement: VersionRequirement,
}

/// No available compiler version satisfies every declared range.
#[derive(Clone, Debug)]
pub struct ResolveError {
    pub requirements: Vec<(PathBuf, String)>,
    pub candidates: Vec<Version>,
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "no available SimplicityHL compiler satisfies every declared version range:"
        )?;
        for (path, range) in &self.requirements {
            writeln!(f, "  {} requires `{}`", path.display(), range)?;
        }
        let list = self
            .candidates
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "  available: [{list}]")
    }
}

impl std::error::Error for ResolveError {}

/// Reads a file's `simc` directive without compiling it (`None` if absent).
pub fn requirement_of(content: &str) -> Result<Option<VersionRequirement>, String> {
    SimcDirective::requirement_of(content)
}

/// The source with its `simc` directive cut out, so directive-unaware parsers
/// (which lex `simc` as a keyword) can read it.
pub fn without_directive(source: &str) -> Cow<'_, str> {
    match SimcDirective::span_of(source) {
        Ok(Some(span)) => Cow::Owned(format!("{}{}", &source[..span.start], &source[span.end..])),
        _ => Cow::Borrowed(source),
    }
}

/// Reads the `simc` directive of every `.simf` file under `src_dir` matching `patterns`.
pub fn collect_requirements(src_dir: &Path, patterns: &[String]) -> Result<Vec<FileRequirement>, BuildError> {
    let walker = globwalk::GlobWalkerBuilder::from_patterns(src_dir, patterns)
        .follow_links(true)
        .file_type(FileType::FILE)
        .build()?
        .filter_map(Result::ok);

    let mut requirements = Vec::new();
    for entry in walker {
        let path = entry.path().to_path_buf();
        let content = std::fs::read_to_string(&path)?;
        match requirement_of(&content) {
            Ok(Some(requirement)) => requirements.push(FileRequirement { path, requirement }),
            Ok(None) => {}
            Err(error) => return Err(BuildError::InvalidSimcDirective { file: path, error }),
        }
    }
    Ok(requirements)
}

/// Picks the newest candidate satisfying every requirement. Pre-releases are never
/// auto-selected — a range cannot name one — so they install only via an exact pin.
pub fn resolve(requirements: &[FileRequirement], candidates: &[Version]) -> Result<Version, ResolveError> {
    let mut acceptable: Vec<&Version> = candidates
        .iter()
        .filter(|version| version.pre.is_empty())
        .filter(|version| requirements.iter().all(|req| req.requirement.matches(version)))
        .collect();

    // Ascending, so the newest satisfying candidate is last.
    acceptable.sort();
    if let Some(selected) = acceptable.last() {
        return Ok((*selected).clone());
    }

    Err(ResolveError {
        requirements: requirements
            .iter()
            .map(|req| (req.path.clone(), req.requirement.req().to_string()))
            .collect(),
        candidates: candidates.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(path: &str, range: &str) -> FileRequirement {
        FileRequirement {
            path: PathBuf::from(path),
            requirement: VersionRequirement::parse(range).expect("valid range"),
        }
    }

    fn versions(list: &[&str]) -> Vec<Version> {
        list.iter().map(|v| Version::parse(v).unwrap()).collect()
    }

    #[test]
    fn requirement_of_reads_directive() {
        let got = requirement_of("simc \">=0.7.0\";\nfn main() {}").unwrap();
        assert_eq!(got.unwrap().req(), &semver::VersionReq::parse(">=0.7.0").unwrap());
        assert_eq!(requirement_of("fn main() {}").unwrap(), None);
    }

    #[test]
    fn without_directive_strips_leading_directive() {
        assert_eq!(
            without_directive("simc \">=0.7.0\";\nfn main() {}").trim_start(),
            "fn main() {}"
        );
        assert_eq!(without_directive("fn main() {}"), "fn main() {}");
    }

    #[test]
    fn without_directive_preserves_comment_mentioning_simc() {
        let source = "// simc; see docs\nsimc \">=0.7.0\";\nfn main() {}";
        let stripped = without_directive(source);
        assert_eq!(stripped, "// simc; see docs\n\nfn main() {}");
        assert!(matches!(requirement_of(&stripped), Ok(None)), "directive must be gone");
    }

    #[test]
    fn resolve_picks_newest_satisfying() {
        let selected = resolve(&[req("main.simf", ">=0.7.0")], &versions(&["0.6.0", "0.7.0", "0.8.0"])).unwrap();
        assert_eq!(selected, Version::parse("0.8.0").unwrap());
    }

    #[test]
    fn resolve_respects_exact_pin() {
        let selected = resolve(&[req("main.simf", "=0.7.0")], &versions(&["0.7.0", "0.8.0"])).unwrap();
        assert_eq!(selected, Version::parse("0.7.0").unwrap());
    }

    #[test]
    fn resolve_intersects_across_files() {
        let selected = resolve(
            &[req("main.simf", ">=0.7.0"), req("lib.simf", "<0.8.0")],
            &versions(&["0.7.0", "0.8.0"]),
        )
        .unwrap();
        assert_eq!(selected, Version::parse("0.7.0").unwrap());
    }

    #[test]
    fn resolve_excludes_prereleases() {
        let selected = resolve(&[req("main.simf", ">=0.7.0")], &versions(&["0.7.0", "0.8.0-rc.1"])).unwrap();
        assert_eq!(selected, Version::parse("0.7.0").unwrap());

        assert!(resolve(&[req("main.simf", ">=0.7.0")], &versions(&["0.8.0-rc.1"])).is_err());
    }

    #[test]
    fn resolve_reports_conflict() {
        let err = resolve(
            &[req("main.simf", ">=0.8.0"), req("lib.simf", "<0.8.0")],
            &versions(&["0.7.0", "0.8.0"]),
        )
        .unwrap_err();
        let text = err.to_string();
        assert!(text.contains("main.simf"), "{text}");
        assert!(text.contains("lib.simf"), "{text}");
    }
}
