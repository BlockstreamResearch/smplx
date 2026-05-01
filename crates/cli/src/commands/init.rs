use std::{fs, fs::OpenOptions, io::Write, path::Path};

use crate::commands::error::CommandError;
use crate::commands::error::InitError;
use crate::config::INIT_CONFIG;

pub const SIMPLEX_CRATE_NAME: &str = "smplx-std";

pub struct Init;

impl Init {
    pub fn run(smplx_conf_path: impl AsRef<Path>) -> Result<(), CommandError> {
        Self::generate_lib_inplace(&smplx_conf_path)?;

        Self::fill_simplex_toml(smplx_conf_path)?;

        Ok(())
    }

    fn fill_simplex_toml(config_path: impl AsRef<Path>) -> Result<(), InitError> {
        let path_to_write = config_path.as_ref();
        Self::write_to_file(path_to_write, INIT_CONFIG)?;

        println!("Config written to: '{}'", path_to_write.display());

        Ok(())
    }

    fn generate_lib_inplace(config_path: impl AsRef<Path>) -> Result<(), InitError> {
        let pwd = config_path.as_ref().parent().unwrap();
        let name = Self::get_project_name(pwd)?;

        // Create `Cargo.toml` file
        let manifest = {
            let mut manifest = toml_edit::DocumentMut::new();
            manifest["package"] = toml_edit::Item::Table(toml_edit::Table::new());
            manifest["package"]["name"] = toml_edit::value(name);
            manifest["package"]["version"] = toml_edit::value("0.1.0");
            manifest["package"]["edition"] = toml_edit::value("2024");

            let mut dep_table = toml_edit::Table::default();
            dep_table.insert(
                SIMPLEX_CRATE_NAME,
                toml_edit::Item::Value(toml_edit::Value::String(toml_edit::Formatted::new(
                    Self::get_smplx_max_version()?,
                ))),
            );

            manifest["dependencies"] = toml_edit::Item::Table(dep_table);
            manifest
        };

        let default_lib_rs_file_content: &[u8] = { b"pub mod artifacts;" };
        let default_test_file_content: &[u8] = {
            b"\
/// For a complete working example, browse the source at:
/// <https://github.com/BlockstreamResearch/smplx/blob/master/examples/basic/tests/basic_test.rs>
#[simplex::test]
fn dummy_test(context: simplex::TestContext) {
    // your test code here
    todo!()
}"
        };
        let default_p2pk_simf_file_content: &[u8] = {
            b"\
fn main() {
    jet::bip_0340_verify((param::PUBLIC_KEY, jet::sig_all_hash()), witness::SIGNATURE)
}"
        };
        let default_gitignore_file_content: &[u8] = { b"src/artifacts" };

        let manifest_path = pwd.join("Cargo.toml");
        let lib_rs_path = pwd.join("src/lib.rs");
        let p2pk_simf_content = pwd.join("simf/p2pk.simf");
        let test_rs_path = pwd.join("tests/p2pk_test.rs");
        let gitignore_path = pwd.join(".gitignore");

        Self::write_to_file(manifest_path, manifest.to_string())?;
        Self::write_to_file(&lib_rs_path, default_lib_rs_file_content)?;
        Self::write_to_file(&test_rs_path, default_test_file_content)?;
        Self::write_to_file(&p2pk_simf_content, default_p2pk_simf_file_content)?;
        Self::write_to_file(&gitignore_path, default_gitignore_file_content)?;

        Self::execute_cargo_fmt(lib_rs_path)?;

        Ok(())
    }

    fn get_project_name(path: &Path) -> Result<String, InitError> {
        let file_name = path
            .file_name()
            .ok_or_else(|| InitError::PackageName(path.to_path_buf()))?;

        let file_name = file_name
            .to_str()
            .ok_or_else(|| InitError::NonUnicodeName(format!("{file_name:?}")))?;

        Ok(format!("simplex_{}", file_name))
    }

    fn get_smplx_max_version() -> Result<String, InitError> {
        let url = format!("https://crates.io/api/v1/crates/{}", SIMPLEX_CRATE_NAME);

        let response = minreq::get(&url)
            .with_header("User-Agent", "simplex_generator")
            .send()
            .map_err(|e| InitError::CratesIoFetch(format!("Failed to fetch crate info: {}", e)))?;

        let body = response
            .as_str()
            .map_err(|e| InitError::CratesIoFetch(format!("Invalid response body: {}", e)))?;

        let json: serde_json::Value =
            serde_json::from_str(body).map_err(|e| InitError::CratesIoFetch(format!("Failed to parse JSON: {}", e)))?;

        let latest_version = json["crate"]["max_stable_version"]
            .as_str()
            .ok_or_else(|| InitError::CratesIoFetch("Could not find max_version in response".to_string()))?;

        Ok(latest_version.to_string())
    }

    fn write_to_file(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> Result<(), InitError> {
        let path = path.as_ref();

        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| InitError::ResolveParent(path.to_path_buf()))?,
        )
        .map_err(|e| InitError::CreateDirs(e, path.to_path_buf()))?;

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(|e| InitError::OpenFile(e, path.to_path_buf()))?;
        file.write_all(content.as_ref())
            .map_err(|e| InitError::WriteToFile(e, path.to_path_buf()))?;
        file.flush()
            .map_err(|e| InitError::WriteToFile(e, path.to_path_buf()))?;

        Ok(())
    }

    fn execute_cargo_fmt(file: impl AsRef<Path>) -> Result<(), InitError> {
        let mut cargo_test_command = std::process::Command::new("sh");

        cargo_test_command.args(["-c".to_string(), format!("rustfmt {}", file.as_ref().display())]);

        let _output = cargo_test_command.output().map_err(InitError::FmtError);

        Ok(())
    }
}
