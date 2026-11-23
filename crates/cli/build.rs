use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() -> io::Result<()> {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"))
        .join("../../skills")
        .canonicalize()?;
    println!("cargo:rerun-if-changed={}", root.display());
    if !root.join("SKILL.md").is_file() {
        return Err(io::Error::other("skills/SKILL.md is required"));
    }

    let mut files = Vec::new();
    collect_files(&root, &mut files)?;
    files.sort();
    let mut source = String::from("const SKILL_FILES: &[(&str, &[u8], bool)] = &[\n");
    for path in files {
        let relative = path.strip_prefix(&root).expect("bundled file inside skills");
        let relative = relative.to_str().expect("UTF-8 skill filename").replace('\\', "/");
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(&path)?.permissions().mode() & 0o111 != 0
        };
        #[cfg(not(unix))]
        let executable = false;
        source.push_str(&format!("({relative:?}, include_bytes!({path:?}), {executable}),\n"));
    }
    source.push_str("];\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    fs::write(out.join("skill_bundle.rs"), source)
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_files(&entry.path(), files)?;
        } else if kind.is_file() {
            files.push(entry.path());
        } else {
            return Err(io::Error::other(format!(
                "unsupported skill resource: {} (expected a regular file or directory)",
                entry.path().display()
            )));
        }
    }
    Ok(())
}
