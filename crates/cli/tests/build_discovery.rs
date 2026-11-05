use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use smplx_cli::config::CONFIG_FILENAME;

const MALFORMED_SOURCE: &str =
    "fn main() {\n    match false { false => (), true => panic!(), }\n    let x: u8 = 1;\n}\n";

struct Project(PathBuf);

impl Project {
    fn new(name: &str, sources: &[(&str, &str)]) -> Result<Self, Box<dyn std::error::Error>> {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("smplx-build-{name}-{}-{suffix}", std::process::id()));
        fs::create_dir_all(path.join("simf"))?;
        fs::create_dir_all(path.join("src/artifacts"))?;
        fs::write(path.join(CONFIG_FILENAME), "")?;
        for (name, source) in sources {
            fs::write(path.join("simf").join(name), source)?;
        }
        Ok(Self(path))
    }

    fn build(&self) -> std::io::Result<Output> {
        Command::new(env!("CARGO_BIN_EXE_simplex"))
            .arg("build")
            .current_dir(&self.0)
            .output()
    }

    fn metadata(&self) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(&fs::read(
            self.0.join("src/artifacts/metadata.json"),
        )?)?)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn malformed_entrypoint_fails_with_a_source_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let project = Project::new("malformed", &[("contract.simf", MALFORMED_SOURCE)])?;
    let output = project.build()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "build incorrectly succeeded: {stderr}");
    assert!(stderr.contains("contract.simf"), "{stderr}");
    assert!(stderr.contains("Expected `;` after statement"), "{stderr}");
    assert!(!project.0.join("src/artifacts/metadata.json").exists());
    Ok(())
}

#[test]
fn malformed_source_prevents_a_partial_build() -> Result<(), Box<dyn std::error::Error>> {
    let project = Project::new(
        "mixed",
        &[("contract.simf", MALFORMED_SOURCE), ("good.simf", "fn main() {}")],
    )?;
    let metadata = project.0.join("src/artifacts/metadata.json");
    fs::write(&metadata, "previous build metadata\n")?;
    let output = project.build()?;

    assert!(
        !output.status.success(),
        "build incorrectly succeeded: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(metadata)?, "previous build metadata\n");
    assert!(!project.0.join("src/artifacts/good.rs").exists());
    Ok(())
}

#[test]
fn repaired_entrypoint_builds() -> Result<(), Box<dyn std::error::Error>> {
    let repaired = MALFORMED_SOURCE.replace("}\n    let", "};\n    let");
    let project = Project::new("repaired", &[("contract.simf", &repaired)])?;
    let output = project.build()?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(project.metadata()?["sources"]["contract.simf"].is_object());
    Ok(())
}

#[test]
fn valid_helpers_without_main_are_skipped() -> Result<(), Box<dyn std::error::Error>> {
    let project = Project::new(
        "helper",
        &[("good.simf", "fn main() {}"), ("helper.simf", "pub fn helper() {}")],
    )?;
    let output = project.build()?;

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let metadata = project.metadata()?;
    assert!(metadata["sources"]["good.simf"].is_object());
    assert_eq!(metadata["sources"].as_object().unwrap().len(), 1);
    Ok(())
}
