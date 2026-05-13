use std::path::Path;
use std::process::Command;
use std::{fmt::Display, fs, path::PathBuf};

use smplx_build::config::DEFAULT_DEPENDENCY_DIR;
use smplx_build::{ArtifactsResolver, BuildConfig, DependencyConfig};

use crate::commands::error::InstallError;
use crate::commands::error::CommandError;
use crate::config::Config;

pub struct Install;

pub struct InstalledRepos(Vec<PathBuf>);

impl Install {
    pub fn run(config: BuildConfig, deps: DependencyConfig) -> Result<(), CommandError> {
        let mut installed_repos = Vec::<PathBuf>::new();

        let deps_dir = Path::new(&config.src_dir).join(DEFAULT_DEPENDENCY_DIR);
        fs::create_dir_all(deps_dir.clone())?;

        Self::install_repos(deps, &deps_dir, &mut installed_repos)?;
        let installed_repos = InstalledRepos(installed_repos);

        println!("Installed repos: {installed_repos}");

        Ok(())
    }

    fn install_repos(
        deps: DependencyConfig, 
        deps_dir: &Path, 
        installed_repos: &mut Vec<PathBuf>
    ) -> Result<(), InstallError> {
        for (_, dependency) in deps.inner.iter() {
            
            let Some(git_repo_url) = &dependency.git else {
                continue; 
            };
                
            let hashed_folder = ArtifactsResolver::generate_hashed_repo_path(git_repo_url)
                .ok_or_else(|| InstallError::InvalidUrl(git_repo_url.clone()))?;

            let target_dir = deps_dir.join(hashed_folder);

            if !target_dir.exists() {
                fs::create_dir_all(&target_dir)
                    .map_err(|e| InstallError::CreateDir(e, target_dir.clone()))?;
            }

            let is_empty = fs::read_dir(&target_dir)
                .map_err(|e| InstallError::ReadDir(e, target_dir.clone()))?
                .next()
                .is_none();

            // Consider it installed and skip cloning.
            if is_empty {
                // Assumes the folder is perfectly empty
                let status = Command::new("git")
                    .arg("clone")
                    .arg("--depth")
                    .arg("1")
                    .arg(git_repo_url)
                    .arg(&target_dir)
                    .status()
                    .map_err(|e| InstallError::GitExecution(e, git_repo_url.clone()))?;

                if !status.success() {
                    // Force to remove file, if something went wrong
                    let _ = std::fs::remove_dir_all(&target_dir);
                    return Err(InstallError::GitCloneFailed(git_repo_url.clone()));
                }
                installed_repos.push(target_dir.clone());
            }

            let config_path = Config::get_path(&target_dir)?;
            let loaded_config = Config::load(config_path)?;

            Self::install_repos(loaded_config.dependencies, deps_dir, installed_repos)?;
        }

        Ok(())
    }
}

impl Display for InstalledRepos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let paths_len = self.0.len();
        let mut result = String::from("[");

        for (index, path) in self.0.iter().enumerate() {
            result.push_str(&format!("\n    {}", path.display()));

            if index < paths_len - 1 {
                result.push(',');
            } else {
                result.push('\n');
            }
        }

        result.push(']');

        write!(f, "{}", result)
    }
}
