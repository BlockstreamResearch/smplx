use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn simplex(work: &Path, home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_simplex"));
    command
        .args(["skill", "path"])
        .current_dir(work)
        .env("SIMPLEX_DIR", home)
        .env_remove("XDG_CONFIG_HOME");
    command
}

fn skill_path(command: &mut Command) -> PathBuf {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1, "stdout must contain only the path");
    let path = PathBuf::from(stdout.trim_end());
    assert!(path.is_absolute());
    assert!(path.join("SKILL.md").is_file());
    path
}

fn assert_same_tree(source: &Path, extracted: &Path) {
    let mut source_names = Vec::new();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        source_names.push(entry.file_name());
        let target = extracted.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            assert_same_tree(&entry.path(), &target);
        } else {
            assert_eq!(fs::read(entry.path()).unwrap(), fs::read(&target).unwrap());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    fs::metadata(entry.path()).unwrap().permissions().mode() & 0o111,
                    fs::metadata(target).unwrap().permissions().mode() & 0o111
                );
            }
        }
    }
    source_names.sort();
    let mut extracted_names = fs::read_dir(extracted)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    extracted_names.sort();
    assert_eq!(source_names, extracted_names);
}

#[test]
fn relocated_binary_extracts_the_entire_root_skill_without_a_project() {
    let temp = tempfile::tempdir().unwrap();
    let binary = temp.path().join("simplex");
    fs::copy(env!("CARGO_BIN_EXE_simplex"), &binary).unwrap();
    let home = temp.path().join("installation with spaces");
    let mut command = Command::new(binary);
    command
        .args(["skill", "path"])
        .current_dir(temp.path())
        .env("SIMPLEX_DIR", &home);
    let path = skill_path(&mut command);
    assert!(path.starts_with(home.canonicalize().unwrap()));
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills");
    assert_same_tree(&source, &path);

    let metadata = fs::metadata(path.join("SKILL.md")).unwrap();
    assert_eq!(path, skill_path(&mut simplex(temp.path(), &home)));
    assert_eq!(
        metadata.modified().unwrap(),
        fs::metadata(path.join("SKILL.md")).unwrap().modified().unwrap()
    );
}

#[test]
fn simultaneous_calls_publish_one_complete_bundle() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("installation");
    let handles = (0..4)
        .map(|_| {
            let work = temp.path().to_path_buf();
            let home = home.clone();
            std::thread::spawn(move || skill_path(&mut simplex(&work, &home)))
        })
        .collect::<Vec<_>>();
    let paths = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert!(paths.iter().all(|path| path == &paths[0]));
    assert_same_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills"), &paths[0]);
    assert_eq!(fs::read_dir(home.join("skills")).unwrap().count(), 1);
}

#[test]
fn modified_resources_fail_without_overwriting_local_content() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("installation");
    let path = skill_path(&mut simplex(temp.path(), &home));
    let resource = path.join("references/testing.md");
    fs::write(&resource, "local changes").unwrap();
    let output = simplex(temp.path(), &home).output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing or modified"));
    assert_eq!(fs::read_to_string(resource).unwrap(), "local changes");
}

#[test]
fn extraction_error_does_not_print_a_path() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("not-a-directory");
    fs::write(&home, "keep").unwrap();
    let output = simplex(temp.path(), &home).output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn installer_environment_fallbacks_and_relative_override_return_absolute_paths() {
    let temp = tempfile::tempdir().unwrap();
    let mut command = simplex(temp.path(), temp.path());
    command.env_remove("SIMPLEX_DIR").env("HOME", temp.path());
    let home_path = skill_path(&mut command);
    assert!(home_path.starts_with(temp.path().join(".simplex").canonicalize().unwrap()));

    let xdg = temp.path().join("config");
    command.env("XDG_CONFIG_HOME", &xdg);
    let xdg_path = skill_path(&mut command);
    assert!(xdg_path.starts_with(xdg.join(".simplex").canonicalize().unwrap()));

    command.env("SIMPLEX_DIR", "relative-installation");
    let relative_path = skill_path(&mut command);
    assert!(relative_path.starts_with(temp.path().join("relative-installation").canonicalize().unwrap()));

    command
        .env_remove("SIMPLEX_DIR")
        .env_remove("HOME")
        .env_remove("XDG_CONFIG_HOME");
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn skill_has_only_the_path_subcommand() {
    use clap::Parser;
    use smplx_cli::Cli;

    assert!(Cli::try_parse_from(["simplex", "skill", "path"]).is_ok());
    for args in [
        vec!["simplex", "skill"],
        vec!["simplex", "skill", "show"],
        vec!["simplex", "skill", "path", "extra"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}
