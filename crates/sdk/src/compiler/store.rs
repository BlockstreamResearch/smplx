//! The content-addressed `simc` store and version resolution.

use std::path::PathBuf;

use super::{CompilerError, lockfile};

/// The compiler store root: `SIMPLEX_COMPILERS` if set, else `~/.simplex/compilers`.
///
/// # Errors
/// Returns [`CompilerError::NoStore`] if the store root cannot be located.
pub fn compilers_dir() -> Result<PathBuf, CompilerError> {
    if let Some(dir) = std::env::var_os("SIMPLEX_COMPILERS").filter(|dir| !dir.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME").ok_or(CompilerError::NoStore)?;
    Ok(PathBuf::from(home).join(".simplex/compilers"))
}

/// Where `version` lives in the store (present or not): `<store>/<version>/bin/simc`.
///
/// # Errors
/// Returns [`CompilerError::NoStore`] if the store root cannot be located.
pub fn simc_path(version: &str) -> Result<PathBuf, CompilerError> {
    Ok(compilers_dir()?.join(version).join("bin").join("simc"))
}

/// This host's release asset (`simc-<os>-<arch>.tar.gz`), matching the compiler CI.
///
/// # Errors
/// Returns [`CompilerError::UnsupportedPlatform`] on an OS/arch with no prebuilt.
pub fn host_asset() -> Result<String, CompilerError> {
    let os = match std::env::consts::OS {
        "macos" => "macos",
        "linux" => "linux",
        other => return Err(CompilerError::UnsupportedPlatform(other.to_string())),
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        other => return Err(CompilerError::UnsupportedPlatform(other.to_string())),
    };
    Ok(format!("simc-{os}-{arch}.tar.gz"))
}

/// The installed versions (directories containing a `bin/simc`), newest first; an
/// absent store reads as empty.
///
/// # Errors
/// Returns [`CompilerError::NoStore`] if the store root cannot be located.
pub fn installed_versions() -> Result<Vec<String>, CompilerError> {
    let root = compilers_dir()?;
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Ok(Vec::new());
    };

    let mut versions: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("bin").join("simc").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

    versions.sort_by(|a, b| match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(a), Ok(b)) => b.cmp(&a),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => a.cmp(b),
    });
    Ok(versions)
}

/// The version to use when a program does not pin one: `SIMPLEX_COMPILER_VERSION`,
/// else the nearest `simplex.lock`. Deliberately no "newest in the store" fallback —
/// the compiler version determines the program's address.
///
/// # Errors
/// Returns [`CompilerError::NoVersion`] if none of the above yields a version.
pub fn resolve_version() -> Result<String, CompilerError> {
    match std::env::var("SIMPLEX_COMPILER_VERSION") {
        Ok(version) if !version.trim().is_empty() => return Ok(version),
        _ => {}
    }

    if let Some(lock) = nearest_lock() {
        return Ok(lock.version);
    }

    Err(CompilerError::NoVersion)
}

/// The hash the nearest `simplex.lock` records for this host, provided it pins
/// exactly `version` — a lock for some other version says nothing about this one.
pub(super) fn lock_sha256_for(version: &str) -> Option<String> {
    let lock = nearest_lock()?;
    if lock.version != version {
        return None;
    }
    lock.sha256_for_host()
}

/// The nearest `simplex.lock`, walking up from the current directory.
fn nearest_lock() -> Option<lockfile::LockFile> {
    lockfile::nearest(&std::env::current_dir().ok()?)
}
