use std::fmt::{Display, Formatter, Result as FmtResult};
use std::path::Path;
use std::process::Command;
use std::{fs, path::PathBuf};

use smplx_build::config::{DEFAULT_DEPENDENCY_DIR, Dependency, GitRef};
use smplx_build::{ArtifactsResolver, DependencyConfig};

use crate::commands::error::CommandError;
use crate::commands::error::InstallError;
use crate::config::Config;

pub struct Install;

pub struct InstalledRepos(Vec<PathBuf>);

impl Install {
    /// Installs all git-based dependencies into the local [`DEFAULT_DEPENDENCY_DIR`] directory.
    ///
    /// Clones each dependency declared in [`DEFAULT_DEPENDENCY_DIR`] that specifies a `git` source,
    /// then reports which repositories were installed.
    ///
    /// # Errors
    ///
    /// Returns a [`CommandError`] if:
    /// - The [`DEFAULT_DEPENDENCY_DIR`] directory cannot be created (e.g. permission denied).
    /// - Any repository fails to clone or install.
    pub fn run(deps: &DependencyConfig) -> Result<(), CommandError> {
        let mut installed_repos = Vec::<PathBuf>::new();

        let deps_dir = Path::new(DEFAULT_DEPENDENCY_DIR);
        fs::create_dir_all(DEFAULT_DEPENDENCY_DIR)?;

        Self::install_repos(deps, deps_dir, &mut installed_repos)?;
        let installed_repos = InstalledRepos(installed_repos);

        if installed_repos.0.is_empty() {
            println!("No repositories were installed.");
        } else {
            println!("Installed repositories: {installed_repos}");
        }

        Ok(())
    }

    fn install_repos(
        deps: &DependencyConfig,
        deps_dir: &Path,
        installed_repos: &mut Vec<PathBuf>,
    ) -> Result<(), InstallError> {
        for dependency in deps.inner.values() {
            let (git_repo_url, reference) = match dependency {
                Dependency::Git { url, reference } => (url, reference),
                Dependency::Path(_) => continue,
            };

            let hashed_dir = ArtifactsResolver::generate_hashed_repo_path(git_repo_url, reference.as_ref())
                .ok_or_else(|| InstallError::InvalidUrl(git_repo_url.clone()))?;

            let target_dir = deps_dir.join(hashed_dir);

            if !target_dir.exists() {
                fs::create_dir_all(&target_dir).map_err(|e| InstallError::CreateDir(e, target_dir.clone()))?;
            }

            Self::clone_repo(git_repo_url, reference.as_ref(), &target_dir, installed_repos)?;

            let config_path = Config::get_path(&target_dir)?;
            let loaded_config = Config::load(config_path)?;

            Self::install_repos(&loaded_config.dependencies, deps_dir, installed_repos)?;
        }

        Ok(())
    }

    fn clone_repo(
        url: &str,
        reference: Option<&GitRef>,
        target_dir: &Path,
        installed_repos: &mut Vec<PathBuf>,
    ) -> Result<(), InstallError> {
        let is_empty = fs::read_dir(target_dir)
            .map_err(|err| InstallError::ReadDir(err, target_dir.to_path_buf()))?
            .next()
            .is_none();

        // Consider it installed and skip cloning.
        if !is_empty {
            return Ok(());
        }

        let target_str = target_dir.to_str().unwrap_or_default();

        let execute_git = |args: &[&str]| -> Result<(), InstallError> {
            let status = Command::new("git")
                .args(args)
                .status()
                .map_err(|err| InstallError::GitExecution(err, url.to_owned()))?;

            if !status.success() {
                // Force to remove file, if something went wrong
                let _ = std::fs::remove_dir_all(target_dir);
                return Err(InstallError::GitCloneFailed(url.to_owned()));
            }

            Ok(())
        };

        match reference {
            Some(GitRef::Tag(tag)) => {
                execute_git(&["clone", "--depth", "1", "--branch", tag.as_str(), url, target_str])?;
            }
            Some(GitRef::Rev(rev)) => {
                execute_git(&["clone", url, target_str])?;
                execute_git(&["-C", target_str, "checkout", rev.as_str()])?;
            }
            None => {
                execute_git(&["clone", "--depth", "1", url, target_str])?;
            }
        }

        installed_repos.push(target_dir.to_path_buf());

        Ok(())
    }
}

impl Display for InstalledRepos {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "[")?;

        for (index, path) in self.0.iter().enumerate() {
            if index > 0 {
                write!(f, ",")?;
            }
            write!(f, "\n    {}", path.display())?;
        }

        if !self.0.is_empty() {
            writeln!(f)?;
        }

        write!(f, "]")
    }
}
