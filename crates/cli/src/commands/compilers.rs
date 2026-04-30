use smplx_build::compiler::Simc;

use crate::commands::error::CommandError;

pub struct Compilers {}

impl Compilers {
    pub fn run() -> Result<(), CommandError> {
        let versions = Simc::list_downloaded_versions()?;

        if versions.is_empty() {
            println!("No Simplicity compilers are currently installed.");
        } else {
            println!("Installed Simplicity compilers:");
            for v in versions {
                println!("  - v{}", v);
            }
        }

        Ok(())
    }
}
