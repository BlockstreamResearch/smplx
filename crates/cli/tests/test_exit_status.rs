#[cfg(unix)]
#[test]
fn simplex_test_propagates_failed_test_runner_status() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let test_dir = std::env::temp_dir().join(format!("smplx-cli-exit-status-{suffix}"));
    let bin_dir = test_dir.join("bin");
    fs::create_dir_all(&bin_dir)?;
    fs::write(test_dir.join("Simplex.toml"), "")?;

    let nextest = bin_dir.join("smplx-nextest");
    fs::write(&nextest, "#!/bin/sh\nexit 7\n")?;
    let mut permissions = fs::metadata(&nextest)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&nextest, permissions)?;

    let mut path_entries = vec![bin_dir];
    if let Some(path) = std::env::var_os("PATH") {
        path_entries.extend(std::env::split_paths(&path));
    }

    let output = Command::new(env!("CARGO_BIN_EXE_simplex"))
        .arg("test")
        .current_dir(&test_dir)
        .env("PATH", std::env::join_paths(path_entries)?)
        .output()?;

    assert_ne!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("test command failed with exit status 7")
            || stderr.contains("test command failed with exit status 7"),
        "stdout: {stdout}\nstderr: {stderr}"
    );

    fs::remove_dir_all(test_dir)?;
    Ok(())
}
