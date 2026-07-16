//! Running `simc` to compile a program into an [`Artifact`].

use std::path::Path;
use std::process::Command;

use simplicityhl::Arguments;

use crate::program::artifact::Artifact;

use super::CompilerError;

/// Compiles the program at `entry` into a frozen [`Artifact`]: `sources` are
/// materialized into a scratch directory and `deps` become `--dep` flags re-rooted
/// there, so `simc` resolves every import off disk. `debug` compiles with
/// instrumentation — a different program with a different CMR.
///
/// # Errors
/// Returns [`CompilerError`] if `simc` cannot be run, exits unsuccessfully, or its
/// output cannot be parsed into an artifact.
pub fn compile(
    simc: &Path,
    sources: &[(String, String)],
    entry: &str,
    deps: &[(String, String, String)],
    arguments: &Arguments,
    debug: bool,
) -> Result<Artifact, CompilerError> {
    // Exclusive, unguessable, 0700, removed on drop — no shared /tmp state to race.
    let workdir = tempfile::Builder::new().prefix("smplx-simc-").tempdir()?;

    compile_in(simc, sources, entry, deps, arguments, debug, workdir.path())
}

fn compile_in(
    simc: &Path,
    sources: &[(String, String)],
    entry: &str,
    deps: &[(String, String, String)],
    arguments: &Arguments,
    debug: bool,
    workdir: &Path,
) -> Result<Artifact, CompilerError> {
    for (relative, contents) in sources {
        let path = workdir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents)?;
    }
    // A mapped dir holding no `.simf` files is never created by the writes above;
    // create them all so `simc` can canonicalize every `--dep` path.
    for (context, _, target) in deps {
        std::fs::create_dir_all(workdir.join(context))?;
        std::fs::create_dir_all(workdir.join(target))?;
    }
    let entry_path = workdir.join(entry);

    let mut cmd = Command::new(simc);
    // `-Z imports` enables the module/`crate::` syntax; harmless for single-file
    // programs that use none.
    cmd.arg(&entry_path).arg("--json").arg("-Z").arg("imports");
    if debug {
        cmd.arg("--debug");
    }
    for flag in dep_flags(workdir, deps) {
        cmd.arg("--dep").arg(flag);
    }

    // Only pass --args when there are arguments: an empty file is unnecessary, and
    // this keeps arg-free programs identical to a bare `simc <file> --json`.
    if arguments.iter().next().is_some() {
        let args_json = serde_json::to_string(arguments)
            .map_err(|e| CompilerError::Output(format!("serializing arguments: {e}")))?;
        let args_path = workdir.join("args.json");
        std::fs::write(&args_path, args_json)?;
        cmd.arg("--args").arg(&args_path);
    }

    let output = cmd
        .output()
        .map_err(|e| CompilerError::Invoke(format!("{}: {e}", simc.display())))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CompilerError::Invoke(format!(
            "simc exited {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    let json = String::from_utf8(output.stdout)
        .map_err(|e| CompilerError::Output(format!("simc output is not utf-8: {e}")))?;
    Artifact::from_json(&json).map_err(CompilerError::Output)
}

/// `--dep CONTEXT:ALIAS=TARGET` values for the embedded remappings, re-rooted at
/// `workdir` (`""` is the scratch root itself).
fn dep_flags(workdir: &Path, deps: &[(String, String, String)]) -> Vec<String> {
    deps.iter()
        .map(|(context, alias, target)| {
            format!(
                "{}:{alias}={}",
                workdir.join(context).display(),
                workdir.join(target).display()
            )
        })
        .collect()
}
