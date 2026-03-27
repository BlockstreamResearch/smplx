use std::path::PathBuf;

use crate::commands::InitFlags;
use crate::commands::error::CommandError;
use crate::commands::init::Init;

pub struct New;

impl New {
    pub fn run(name: &str) -> Result<(), CommandError> {
        let project_dir: PathBuf = std::env::current_dir()?.join(name);

        if project_dir.exists() {
            return Err(CommandError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("destination '{}' already exists", project_dir.display()),
            )));
        }

        let simplex_conf_path = project_dir.join("Simplex.toml");

        Init::run(InitFlags { lib: true }, simplex_conf_path)?;

        println!("Created new Simplex project '{}'", name);

        Ok(())
    }
}
