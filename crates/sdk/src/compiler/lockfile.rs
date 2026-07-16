//! The single codec for `simplex.lock`: both the build tool and the runtime read
//! the lock through this module, so the two can never disagree on its format.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};

use super::{CompilerError, host_asset, sha256_hex};

/// The lockfile Simplex writes next to `Simplex.toml`, pinning the compiler.
pub const LOCKFILE: &str = "simplex.lock";

/// A parsed `simplex.lock`. A `fork` field written by older Simplex versions is
/// accepted and ignored.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LockFile {
    /// Binary sha256 keyed by release asset name — one entry per host platform.
    /// A legacy flat-string `sha256` recorded no platform and parses as empty.
    #[serde(default, deserialize_with = "sha256_map_or_legacy")]
    pub sha256: BTreeMap<String, String>,
    /// The pinned compiler version.
    pub version: String,
}

impl LockFile {
    /// The recorded binary hash for this host's platform, if any.
    #[must_use]
    pub fn sha256_for_host(&self) -> Option<String> {
        let asset = host_asset().ok()?;
        self.sha256.get(&asset).cloned()
    }
}

/// The lock at `dir/simplex.lock`, or `None` when it is absent or unreadable.
#[must_use]
pub fn read_at(dir: &Path) -> Option<LockFile> {
    let text = std::fs::read_to_string(dir.join(LOCKFILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// The nearest `simplex.lock`, walking up from `start`.
#[must_use]
pub fn nearest(start: &Path) -> Option<LockFile> {
    let mut dir = start.to_path_buf();
    loop {
        if let Some(lock) = read_at(&dir) {
            return Some(lock);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Writes `dir/simplex.lock` for `version`, preserving other platforms' hash
/// entries so a committed lock accumulates one per platform the team builds on.
///
/// # Errors
/// Returns a [`CompilerError`] if the binary cannot be hashed or the file cannot be
/// written — a lock with a missing hash must never be silently produced.
pub fn write(dir: &Path, version: &str, simc: &Path) -> Result<PathBuf, CompilerError> {
    let mut sha256 = match read_at(dir) {
        Some(existing) if existing.version == version => existing.sha256,
        _ => BTreeMap::new(),
    };
    sha256.insert(host_asset()?, sha256_hex(simc)?);

    let lock = LockFile {
        sha256,
        version: version.to_string(),
    };
    let path = dir.join(LOCKFILE);
    let contents = serde_json::to_string_pretty(&lock)
        .map_err(|e| CompilerError::Output(format!("serializing {LOCKFILE}: {e}")))?;
    std::fs::write(&path, format!("{contents}\n"))?;
    Ok(path)
}

/// Accepts the current `{asset: hex}` object or a legacy flat string (as empty).
fn sha256_map_or_legacy<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    // The legacy payload itself is never read — only its shape matters.
    #[derive(Deserialize)]
    #[serde(untagged)]
    #[allow(dead_code)]
    enum Sha256 {
        PerAsset(BTreeMap<String, String>),
        Legacy(String),
    }

    Ok(match Sha256::deserialize(deserializer)? {
        Sha256::PerAsset(map) => map,
        Sha256::Legacy(_) => BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_and_legacy_locks() {
        let current: LockFile =
            serde_json::from_str(r#"{"sha256":{"simc-macos-aarch64.tar.gz":"ab"},"version":"0.9.0"}"#).unwrap();
        assert_eq!(current.version, "0.9.0");
        assert_eq!(current.sha256.len(), 1);

        let legacy_sha: LockFile = serde_json::from_str(r#"{"sha256":"abcd","version":"0.9.0"}"#).unwrap();
        assert!(legacy_sha.sha256.is_empty(), "legacy flat sha256 records no platform");

        // A lock written before the release source became fixed still has `fork`.
        let legacy_fork: LockFile =
            serde_json::from_str(r#"{"fork":"https://github.com/x/y","sha256":{},"version":"0.9.0"}"#).unwrap();
        assert_eq!(legacy_fork.version, "0.9.0");
    }

    #[test]
    fn version_is_required() {
        assert!(serde_json::from_str::<LockFile>(r#"{"sha256":{}}"#).is_err());
    }
}
