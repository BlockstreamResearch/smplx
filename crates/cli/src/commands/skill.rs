use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::error::CommandError;

include!(concat!(env!("OUT_DIR"), "/skill_bundle.rs"));

/// Prints the skill location without requiring a Simplex project or network access.
///
/// # Errors
/// Returns an error if the installation directory cannot be resolved, the bundle
/// cannot be written, or an existing extracted resource has been changed.
pub fn run() -> Result<(), CommandError> {
    let root = simplex_dir()?.join("skills");
    let path = extract_bundle(&root)?;
    writeln!(io::stdout().lock(), "{}", path.display())?;
    Ok(())
}

fn simplex_dir() -> io::Result<PathBuf> {
    if let Some(dir) = env::var_os("SIMPLEX_DIR").filter(|dir| !dir.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let base = env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .or_else(|| env::var_os("HOME").filter(|dir| !dir.is_empty()))
        .ok_or_else(|| io::Error::other("set SIMPLEX_DIR or HOME to locate the bundled skill"))?;
    // Match simplexup's installation directory, including its XDG override.
    Ok(PathBuf::from(base).join(".simplex"))
}

fn bundle_hash() -> String {
    let mut hash = Sha256::new();
    for (path, content, executable) in SKILL_FILES {
        hash.update((path.len() as u64).to_le_bytes());
        hash.update(path.as_bytes());
        hash.update((content.len() as u64).to_le_bytes());
        hash.update(content);
        hash.update([u8::from(*executable)]);
    }
    hex::encode(hash.finalize())
}

fn extract_bundle(cache: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(cache)?;
    let destination = cache.join(bundle_hash());
    if destination.exists() {
        verify_bundle(&destination)?;
        return destination.canonicalize();
    }

    let staging = tempfile::Builder::new().prefix(".skill-").tempdir_in(cache)?;
    for (relative, content, executable) in SKILL_FILES {
        let file = staging.path().join(relative);
        fs::create_dir_all(file.parent().expect("skill resource parent"))?;
        fs::write(&file, content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if *executable {
                fs::set_permissions(&file, fs::Permissions::from_mode(0o755))?;
            }
        }
        #[cfg(not(unix))]
        let _ = executable;
    }

    // Publish only a complete tree. Another invocation may have installed it first.
    if let Err(error) = fs::rename(staging.path(), &destination)
        && !destination.is_dir()
    {
        return Err(error);
    }
    verify_bundle(&destination)?;
    destination.canonicalize()
}

fn verify_bundle(root: &Path) -> io::Result<()> {
    for (relative, content, _) in SKILL_FILES {
        let file = root.join(relative);
        if fs::read(&file).is_ok_and(|actual| actual == *content) {
            continue;
        }
        return Err(io::Error::other(format!(
            "bundled skill resource '{}' is missing or modified; remove '{}' and retry simplex skill to restore it",
            file.display(),
            root.display()
        )));
    }
    Ok(())
}
