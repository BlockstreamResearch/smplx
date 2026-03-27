use std::path::PathBuf;

use crate::commands::ExampleName;
use crate::commands::error::{CommandError, InitError};
use crate::commands::init::{Init, SIMPLEX_CRATE_NAME};

pub struct Example;

impl Example {
    pub fn run(example: &ExampleName) -> Result<(), CommandError> {
        match example {
            ExampleName::Basic => Self::create_basic()?,
        }
        Ok(())
    }

    fn create_basic() -> Result<(), InitError> {
        let dir: PathBuf = std::env::current_dir().map_err(InitError::FmtError)?.join("basic");

        if dir.exists() {
            return Err(InitError::CreateDirs(
                std::io::Error::new(std::io::ErrorKind::AlreadyExists, "destination 'basic' already exists"),
                dir,
            ));
        }

        let smplx_version = Init::get_smplx_max_version()?;

        // Generate Cargo.toml dynamically with the latest smplx-std version
        let manifest = {
            let mut manifest = toml_edit::DocumentMut::new();
            manifest["package"] = toml_edit::Item::Table(toml_edit::Table::new());
            manifest["package"]["name"] = toml_edit::value("simplex_example");
            manifest["package"]["version"] = toml_edit::value("0.1.0");
            manifest["package"]["edition"] = toml_edit::value("2024");
            manifest["package"]["rust-version"] = toml_edit::value("1.91.0");

            let mut dep_table = toml_edit::Table::default();
            dep_table.insert(
                SIMPLEX_CRATE_NAME,
                toml_edit::Item::Value(toml_edit::Value::String(toml_edit::Formatted::new(smplx_version))),
            );
            dep_table.insert(
                "anyhow",
                toml_edit::Item::Value(toml_edit::Value::String(toml_edit::Formatted::new("1".to_string()))),
            );
            manifest["dependencies"] = toml_edit::Item::Table(dep_table);
            manifest
        };

        Init::write_to_file(dir.join("Cargo.toml"), manifest.to_string())?;
        Init::write_to_file(
            dir.join("Simplex.toml"),
            include_str!("../../../../examples/basic/Simplex.toml"),
        )?;
        Init::write_to_file(
            dir.join(".gitignore"),
            include_str!("../../../../examples/basic/.gitignore"),
        )?;
        Init::write_to_file(
            dir.join("src/lib.rs"),
            include_str!("../../../../examples/basic/src/lib.rs"),
        )?;
        Init::write_to_file(
            dir.join("tests/example_test.rs"),
            include_str!("../../../../examples/basic/tests/example_test.rs"),
        )?;
        Init::write_to_file(
            dir.join("simf/p2pk.simf"),
            include_str!("../../../../examples/basic/simf/p2pk.simf"),
        )?;
        Init::write_to_file(
            dir.join("simf/options.simf"),
            include_str!("../../../../examples/basic/simf/options.simf"),
        )?;
        Init::write_to_file(
            dir.join("simf/module/option_offer.simf"),
            include_str!("../../../../examples/basic/simf/module/option_offer.simf"),
        )?;
        Init::write_to_file(
            dir.join("simf/another_dir/array_tr_storage.simf"),
            include_str!("../../../../examples/basic/simf/another_dir/array_tr_storage.simf"),
        )?;
        Init::write_to_file(
            dir.join("simf/another_dir/another_module/bytes32_tr_storage.simf"),
            include_str!("../../../../examples/basic/simf/another_dir/another_module/bytes32_tr_storage.simf"),
        )?;
        Init::write_to_file(
            dir.join("simf/another_dir/another_module/dual_currency_deposit.simf"),
            include_str!("../../../../examples/basic/simf/another_dir/another_module/dual_currency_deposit.simf"),
        )?;

        println!("Created example project 'basic'");
        println!(
            "Run `simplex build` inside 'basic/' to generate artifacts, then `simplex test integration` to run the tests."
        );

        Ok(())
    }
}
