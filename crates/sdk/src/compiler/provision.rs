//! Downloading, verifying, and locating `simc` binaries.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::store::{compilers_dir, host_asset, lock_sha256_for, simc_path};
use super::{CompilerError, RELEASE_SOURCE, TAG_PREFIX};

/// The `simc` binary for `version`, downloading it if the store lacks it and
/// verifying it against the hash a reachable `simplex.lock` records.
///
/// # Errors
/// Returns [`CompilerError`] if the binary cannot be provisioned or fails verification.
pub fn ensure(version: &str) -> Result<PathBuf, CompilerError> {
    let expected = lock_sha256_for(version);
    provision(version, expected.as_deref()).map_err(|err| match err {
        // These already name the version and the problem.
        err @ (CompilerError::Verify { .. }
        | CompilerError::TooOld { .. }
        | CompilerError::InvalidVersion(_)
        | CompilerError::Invoke(_)
        | CompilerError::NotProvisioned { .. }) => err,
        other => CompilerError::NotProvisioned {
            version: version.to_string(),
            reason: other.to_string(),
        },
    })
}

/// Downloads and unpacks `simc` for `version` into the store, unless already there.
/// The binary is checked against `expected_sha256` (the lock's hash for this host)
/// and, for fresh downloads, the release's `SHA256SUMS`.
///
/// # Errors
/// Returns [`CompilerError`] on an unsupported platform, a failed download or
/// unpack, or a hash mismatch.
pub fn provision(version: &str, expected_sha256: Option<&str>) -> Result<PathBuf, CompilerError> {
    // The version becomes a store path and a URL segment: reject non-semver (a
    // `../…` traversal) and below-floor versions before touching either.
    let requested = semver::Version::parse(version).map_err(|_| CompilerError::InvalidVersion(version.to_string()))?;
    if let Ok(floor) = semver::Version::parse(super::MIN_COMPILER_VERSION)
        && requested < floor
    {
        return Err(CompilerError::TooOld {
            version: version.to_string(),
            min: super::MIN_COMPILER_VERSION,
        });
    }

    let simc = simc_path(version)?;
    if simc.is_file() {
        verify_binary(&simc, version, expected_sha256)?;
        verify_floor(&simc, version)?;
        return Ok(simc);
    }

    let version_dir = compilers_dir()?.join(version);
    let asset = host_asset()?;
    let release = format!("{RELEASE_SOURCE}/releases/download/{TAG_PREFIX}{version}");
    // Stderr: this can run inside a user's program, whose stdout is not ours.
    eprintln!("downloading SimplicityHL compiler {version} from {RELEASE_SOURCE}...");

    // Stage in a sibling temp dir on the same filesystem so the final rename is
    // atomic; exclusively created, removed on drop.
    std::fs::create_dir_all(&version_dir)?;
    let staging = tempfile::Builder::new()
        .prefix(".provision-")
        .tempdir_in(&version_dir)?;
    let staged_bin = staging.path().join("bin");
    std::fs::create_dir_all(&staged_bin)?;

    download_and_extract(&release, &asset, version, expected_sha256, staging.path(), &staged_bin)
        .and_then(|()| promote(&staged_bin, &version_dir.join("bin"), version, expected_sha256))?;

    verify_floor(&simc, version)?;
    Ok(simc)
}

/// Rejects a binary below the floor via the `simc --version` handshake; one that
/// cannot run, answers garbage, or misreports its slot version is broken, not old.
fn verify_floor(simc: &Path, version: &str) -> Result<(), CompilerError> {
    let output = Command::new(simc)
        .arg("--version")
        .output()
        .map_err(|e| CompilerError::Invoke(format!("{}: {e}", simc.display())))?;

    // Pre-versioning compilers predate the handshake and reject the flag.
    if !output.status.success() {
        return Err(CompilerError::TooOld {
            version: version.to_string(),
            min: super::MIN_COMPILER_VERSION,
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let reported = stdout
        .split_whitespace()
        .last()
        .and_then(|v| semver::Version::parse(v).ok())
        .ok_or_else(|| {
            CompilerError::Invoke(format!(
                "{} answered the version handshake with '{}'; the binary looks broken — \
                 remove it from the store and re-provision",
                simc.display(),
                stdout.trim()
            ))
        })?;

    let floor = semver::Version::parse(super::MIN_COMPILER_VERSION).expect("floor is valid semver");
    if reported < floor {
        return Err(CompilerError::TooOld {
            version: version.to_string(),
            min: super::MIN_COMPILER_VERSION,
        });
    }

    // A binary that misreports its slot version is broken; using it would silently
    // compile with the wrong compiler.
    if let Ok(claimed) = semver::Version::parse(version)
        && reported != claimed
    {
        return Err(CompilerError::Invoke(format!(
            "the store claims compiler {version} but {} reports {reported}; \
             remove it from the store and re-provision",
            simc.display()
        )));
    }
    Ok(())
}

fn download_and_extract(
    release: &str,
    asset: &str,
    version: &str,
    expected_sha256: Option<&str>,
    staging: &Path,
    staged_bin: &Path,
) -> Result<(), CompilerError> {
    let url = format!("{release}/{asset}");
    let archive = staging.join(asset);
    std::fs::write(&archive, download(&url)?)?;

    // Verify the archive against the release's published checksums when it has
    // them; their absence is reported but not fatal (older releases predate them).
    match fetch_checksums(release) {
        Some(sums) => verify_archive(&archive, asset, &sums)?,
        None => eprintln!("warning: release {release} publishes no SHA256SUMS; download not verified"),
    }

    let status = Command::new("tar")
        .arg("xzf")
        .arg(&archive)
        .arg("-C")
        .arg(staged_bin)
        .arg("simc")
        .status()
        .map_err(|e| CompilerError::Unpack(e.to_string()))?;
    if !status.success() {
        return Err(CompilerError::Unpack(format!("tar exited {status}")));
    }

    verify_binary(&staged_bin.join("simc"), version, expected_sha256)
}

/// Atomically move the staged `bin` directory into place. Losing the race to a
/// concurrent provision is fine; the winner's binary is verified instead.
fn promote(
    staged_bin: &Path,
    dest_bin: &Path,
    version: &str,
    expected_sha256: Option<&str>,
) -> Result<(), CompilerError> {
    match std::fs::rename(staged_bin, dest_bin) {
        Ok(()) => Ok(()),
        Err(_) if dest_bin.join("simc").is_file() => verify_binary(&dest_bin.join("simc"), version, expected_sha256),
        Err(_) => {
            // A stale `bin/` without a binary (an interrupted install) would wedge
            // every future provision; clear it and retry once.
            std::fs::remove_dir_all(dest_bin)?;
            std::fs::rename(staged_bin, dest_bin)?;
            Ok(())
        }
    }
}

/// The sha256 of a file, as lowercase hex.
///
/// # Errors
/// Returns the IO error if the file cannot be read.
pub fn sha256_hex(path: &Path) -> std::io::Result<String> {
    use simplicityhl::simplicity::hashes::{Hash, sha256};
    let bytes = std::fs::read(path)?;
    Ok(sha256::Hash::hash(&bytes).to_string())
}

/// Check a provisioned binary against the lock's recorded hash, if one is given.
fn verify_binary(simc: &Path, version: &str, expected_sha256: Option<&str>) -> Result<(), CompilerError> {
    let Some(expected) = expected_sha256 else {
        return Ok(());
    };
    let actual = sha256_hex(simc)?;
    if actual != expected {
        return Err(CompilerError::Verify {
            version: version.to_string(),
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}

/// The release's `SHA256SUMS`, or `None` — older releases predate published
/// checksums, so download errors are swallowed.
fn fetch_checksums(release: &str) -> Option<String> {
    let bytes = download(&format!("{release}/SHA256SUMS")).ok()?;
    String::from_utf8(bytes).ok()
}

/// Checks the archive against its `SHA256SUMS` entry (`<hex>  <name>`).
fn verify_archive(archive: &Path, asset: &str, sums: &str) -> Result<(), CompilerError> {
    let expected = sums
        .lines()
        .filter_map(|line| line.split_once("  "))
        .find(|(_, name)| name.trim() == asset)
        .map(|(hex, _)| hex.trim().to_string())
        .ok_or_else(|| CompilerError::Download {
            url: asset.to_string(),
            reason: "release SHA256SUMS has no entry for this asset".to_string(),
        })?;

    let actual = sha256_hex(archive)?;
    if actual != expected {
        return Err(CompilerError::Download {
            url: asset.to_string(),
            reason: format!("archive sha256 {actual} does not match published {expected}"),
        });
    }
    Ok(())
}

/// Fetches `url` into memory (assets are tens of megabytes at most), bounded so a
/// hung server cannot block a lazy runtime compile indefinitely.
fn download(url: &str) -> Result<Vec<u8>, CompilerError> {
    let err = |reason: String| CompilerError::Download {
        url: url.to_string(),
        reason,
    };
    let response = minreq::get(url)
        .with_header("User-Agent", "smplx-sdk")
        .with_timeout(300)
        .send()
        .map_err(|e| err(e.to_string()))?;
    if !(200..300).contains(&response.status_code) {
        return Err(err(format!("HTTP {}", response.status_code)));
    }
    Ok(response.into_bytes())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// A fake `simc` whose `--version` behavior is the given shell-script body.
    fn fake_simc(name: &str, script: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("smplx-fake-simc-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("simc");
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn accepts_versioned_compiler() {
        let simc = fake_simc("v1", "echo 'simc 1.0.0'");
        verify_floor(&simc, "1.0.0").unwrap();
    }

    #[test]
    fn rejects_below_floor_as_too_old() {
        let simc = fake_simc("v065", "echo 'simc 0.6.5'");
        let err = verify_floor(&simc, "0.6.5").unwrap_err();
        assert!(matches!(err, CompilerError::TooOld { .. }), "{err}");
    }

    #[test]
    fn rejects_below_floor_before_downloading() {
        let err = provision("0.6.0", None).unwrap_err();
        assert!(matches!(err, CompilerError::TooOld { .. }), "{err}");
    }

    // A traversal from a hostile lock or env var must be rejected before the
    // version forms a path or URL.
    #[test]
    fn rejects_non_semver_version_before_path_use() {
        for bad in ["../../../tmp/evil", "1.0.0/..", "latest"] {
            let err = provision(bad, None).unwrap_err();
            assert!(matches!(err, CompilerError::InvalidVersion(_)), "{bad}: {err}");
        }
    }

    #[test]
    fn promote_replaces_stale_incomplete_bin() {
        let root = std::env::temp_dir().join(format!("smplx-promote-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let staged = root.join("staging/bin");
        let dest = root.join("bin");
        std::fs::create_dir_all(&staged).unwrap();
        std::fs::create_dir_all(&dest).unwrap(); // stale: exists, but holds no simc
        std::fs::write(staged.join("simc"), "fake").unwrap();

        promote(&staged, &dest, "1.0.0", None).expect("stale bin is replaced");
        assert!(dest.join("simc").is_file(), "binary promoted into place");
        let _ = std::fs::remove_dir_all(&root);
    }

    // A pre-versioning binary (like the real 0.6.0) rejects the flag entirely.
    #[test]
    fn rejects_pre_versioning_binary() {
        let simc = fake_simc("old", "echo 'error: unexpected argument' >&2; exit 2");
        let text = verify_floor(&simc, "0.6.0").unwrap_err().to_string();
        assert!(
            text.contains("0.6.0") && text.contains("too old") && text.contains(super::super::MIN_COMPILER_VERSION),
            "{text}"
        );
    }

    #[test]
    fn broken_handshake_is_not_too_old() {
        let simc = fake_simc("broken", "echo banana");
        let err = verify_floor(&simc, "1.0.0").unwrap_err();
        assert!(matches!(err, CompilerError::Invoke(_)), "{err}");
    }

    #[test]
    fn rejects_version_mismatch() {
        let simc = fake_simc("mismatch", "echo 'simc 0.8.0'");
        let text = verify_floor(&simc, "0.9.0").unwrap_err().to_string();
        assert!(text.contains("0.9.0") && text.contains("0.8.0"), "{text}");
    }
}
