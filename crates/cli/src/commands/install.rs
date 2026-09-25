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
            let (git_repo_url, reference, package) = match dependency {
                Dependency::Git {
                    url,
                    reference,
                    package,
                } => (url, reference, package),
                Dependency::Path(_) => continue,
            };

            let hashed_dir =
                ArtifactsResolver::generate_hashed_repo_path(git_repo_url, reference.as_ref(), package.as_deref())
                    .ok_or_else(|| InstallError::InvalidUrl(git_repo_url.clone()))?;

            let target_dir = deps_dir.join(hashed_dir);

            if !target_dir.exists() {
                fs::create_dir_all(&target_dir).map_err(|e| InstallError::CreateDir(e, target_dir.clone()))?;
            }

            Self::clone_repo(
                git_repo_url,
                reference.as_ref(),
                package.as_deref(),
                &target_dir,
                installed_repos,
            )?;

            let config_path = Config::get_path(&target_dir)?;
            let loaded_config = Config::load(config_path)?;

            Self::install_repos(&loaded_config.dependencies, deps_dir, installed_repos)?;
        }

        Ok(())
    }

    /// Clones `url` into `target_dir`, checking out `reference`.
    /// When `package` is set, only that directory is downloaded and `target_dir` holds
    /// its contents directly, without the rest of the repository or its `.git`.
    fn clone_repo(
        url: &str,
        reference: Option<&GitRef>,
        package: Option<&str>,
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

        // Git keeps a package at its in-repo path, so it is cloned into a sibling
        // staging dir first and then moved out of it.
        let clone_dir = match package {
            Some(_) => {
                let mut staging = target_dir.as_os_str().to_owned();
                staging.push(".tmp");
                let staging = PathBuf::from(staging);

                let _ = fs::remove_dir_all(&staging);
                staging
            }
            None => target_dir.to_path_buf(),
        };

        for mut command in Self::git_commands(url, reference, package, &clone_dir) {
            let output = command
                .output()
                .map_err(|err| InstallError::GitExecution(err, url.to_owned()))?;

            if !output.status.success() {
                // Force to remove file, if something went wrong
                let _ = std::fs::remove_dir_all(&clone_dir);
                return Err(InstallError::GitCloneFailed(url.to_owned()));
            }
        }

        if let Some(package) = package {
            let package_dir = clone_dir.join(package);

            // Git succeeds even if `package` doesn't exist at this ref, leaving an empty checkout.
            let moved = if package_dir.is_dir() {
                // `target_dir` is still empty; `rename` can't replace a directory on every platform.
                fs::remove_dir(target_dir)
                    .and_then(|()| fs::rename(&package_dir, target_dir))
                    .map_err(|err| InstallError::MovePackage(err, target_dir.to_path_buf()))
            } else {
                Err(InstallError::PackageNotFound(package.to_owned(), url.to_owned()))
            };

            let _ = fs::remove_dir_all(&clone_dir);
            moved?;
        }

        installed_repos.push(target_dir.to_path_buf());

        Ok(())
    }

    /// Builds the git commands, in execution order, that clone `url` into `target_dir`
    /// at `reference`, keeping only `package` when it is set.
    ///
    /// 1. Clone the history without contents and any checks.
    /// 2. If `package` is set, restrict the checkout to it.
    /// 3. Check out `rev` (or the cloned ref), downloading only the files it needs.
    fn git_commands(url: &str, reference: Option<&GitRef>, package: Option<&str>, target_dir: &Path) -> Vec<Command> {
        let mut clone = Command::new("git");
        clone.args(["clone", "--filter=blob:none", "--no-checkout"]);

        let checkout_target = match reference {
            Some(GitRef::Rev(rev)) => rev.as_str(),
            Some(GitRef::Tag(name) | GitRef::Branch(name)) => {
                clone.args(["--depth", "1", "--branch", name]);
                "HEAD"
            }
            None => {
                clone.args(["--depth", "1"]);
                "HEAD"
            }
        };
        clone.arg("--").arg(url).arg(target_dir);

        let mut commands = vec![clone];

        if let Some(package) = package {
            let mut sparse = Command::new("git");
            sparse
                .arg("-C")
                .arg(target_dir)
                .args(["sparse-checkout", "set", "--no-cone"])
                .arg(format!("/{package}/"));
            commands.push(sparse);
        }

        let mut checkout = Command::new("git");
        checkout
            .arg("-C")
            .arg(target_dir)
            .args(["checkout", checkout_target, "--"]);
        commands.push(checkout);

        commands
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
