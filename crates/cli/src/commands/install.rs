use std::collections::{HashSet, VecDeque};
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

    /// Installs `deps` and, transitively, the dependencies declared by each installed repository.
    fn install_repos(
        deps: &DependencyConfig,
        deps_dir: &Path,
        installed_repos: &mut Vec<PathBuf>,
    ) -> Result<(), InstallError> {
        let mut visited = HashSet::<PathBuf>::new();
        let mut pending = VecDeque::<PathBuf>::new();

        Self::install_direct_deps(deps, deps_dir, &mut visited, &mut pending, installed_repos)?;

        while let Some(repo_dir) = pending.pop_front() {
            let config_path = Config::get_path(&repo_dir)?;
            let loaded_config = Config::load(config_path)?;

            Self::install_direct_deps(
                &loaded_config.dependencies,
                deps_dir,
                &mut visited,
                &mut pending,
                installed_repos,
            )?;
        }

        Ok(())
    }

    /// Installs the git dependencies listed directly in `deps` that are not yet `visited`.
    fn install_direct_deps(
        deps: &DependencyConfig,
        deps_dir: &Path,
        visited: &mut HashSet<PathBuf>,
        pending: &mut VecDeque<PathBuf>,
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

            if !visited.insert(target_dir.clone()) {
                continue;
            }

            Self::clone_repo(
                git_repo_url,
                reference.as_ref(),
                package.as_deref(),
                &target_dir,
                installed_repos,
            )?;

            pending.push_back(target_dir);
        }

        Ok(())
    }

    /// Clones `url` into `target_dir`, checking out `reference`.
    /// When `package` is set, only that directory is downloaded and `target_dir` holds
    /// its contents directly, without the rest of the repository or its `.git`.
    ///
    /// Git works in a staging directory that becomes `target_dir` only once the checkout is complete,
    /// so an interrupted or concurrent install doesn't leave a partial `target_dir` that looks installed.
    fn clone_repo(
        url: &str,
        reference: Option<&GitRef>,
        package: Option<&str>,
        target_dir: &Path,
        installed_repos: &mut Vec<PathBuf>,
    ) -> Result<(), InstallError> {
        if Self::is_installed(target_dir)? {
            return Ok(());
        }

        // Each process stages separately, so concurrent installs don't clone over each other
        let mut staging_name = std::ffi::OsString::from(".");
        staging_name.push(target_dir.file_name().unwrap_or_default());
        staging_name.push(format!(".{}.tmp", std::process::id()));
        let staging = target_dir.with_file_name(staging_name);

        let _ = fs::remove_dir_all(&staging);
        let installed = Self::stage_and_move(url, reference, package, &staging, target_dir);
        let _ = fs::remove_dir_all(&staging);

        if installed? {
            installed_repos.push(target_dir.to_path_buf());
        }

        Ok(())
    }

    /// Clones into `staging` and moves the checkout, or its `package`, to `target_dir`.
    /// `false` when a concurrent install got there first.
    fn stage_and_move(
        url: &str,
        reference: Option<&GitRef>,
        package: Option<&str>,
        staging: &Path,
        target_dir: &Path,
    ) -> Result<bool, InstallError> {
        for mut command in Self::git_commands(url, reference, package, staging) {
            let output = command
                .output()
                .map_err(|err| InstallError::GitExecution(err, url.to_owned()))?;

            if !output.status.success() {
                return Err(InstallError::GitCloneFailed(url.to_owned()));
            }
        }

        let checkout = match package {
            // Git succeeds even if `package` doesn't exist at this ref, leaving an empty checkout.
            Some(package) if !staging.join(package).is_dir() => {
                return Err(InstallError::PackageNotFound(package.to_owned(), url.to_owned()));
            }
            Some(package) => staging.join(package),
            None => staging.to_path_buf(),
        };

        match fs::rename(&checkout, target_dir) {
            Ok(()) => Ok(true),
            Err(_) if Self::is_installed(target_dir)? => Ok(false),
            Err(err) => Err(InstallError::MoveCheckout(err, target_dir.to_path_buf())),
        }
    }

    fn is_installed(target_dir: &Path) -> Result<bool, InstallError> {
        if target_dir.join(smplx_build::CONFIG_FILENAME).is_file() {
            return Ok(true);
        }

        if target_dir.exists() {
            fs::remove_dir_all(target_dir)
                .map_err(|err| InstallError::RemoveIncomplete(err, target_dir.to_path_buf()))?;
        }

        Ok(false)
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
            Some(GitRef::Rev(rev)) => rev.as_ref(),
            Some(GitRef::Tag(name)) => {
                let qualified_name = format!("refs/tags/{name}");
                clone.args(["--depth", "1", "--revision", &qualified_name]);
                "HEAD"
            }
            Some(GitRef::Branch(name)) => {
                let qualified_name = format!("refs/heads/{name}");
                clone.args(["--depth", "1", "--revision", &qualified_name]);
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
            .args(["checkout", "--detach", checkout_target, "--"]);
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

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://example.invalid/org/repo.git";
    const TARGET: &str = r#"/unused/"git fixture""#;

    fn assert_command(command: &Command, args: &[&str]) {
        assert_eq!(command.get_program(), "git");

        let actual: Vec<_> = command.get_args().map(|arg| arg.to_str().unwrap()).collect();

        assert_eq!(actual, args);
    }

    #[test]
    fn git_commands_revisions_appended_correctly() {
        for rev in [
            "dae86e1950b1277e545cee180551750029cfe735",
            "dae86e",
            "v1.7.4.2-679-g3bee7fb",
            "master",
            "refs/heads/master",
            "HEAD",
            "@",
            "master@{yesterday}",
            "HEAD@{5 minutes ago}",
            "master@{1}",
            "@{1}",
            "@{-1}",
            "master@{upstream}",
            "@{push}",
            "HEAD^",
            "v1.5.1^0",
            "master~3",
            "v0.99.8^{commit}",
            "v0.99.8^{object}",
            "v0.99.8^{tag}",
            "v0.99.8^{}",
            "HEAD^{/fix nasty bug}",
            ":/fix nasty bug",
            ":/^foo",
            ":/!-foo",
            ":/!!foo",
        ] {
            let reference = GitRef::Rev(rev.into());
            let commands = Install::git_commands(URL, Some(&reference), None, Path::new(TARGET));

            assert_eq!(commands.len(), 2, "revision: {rev}");
            let (clone, checkout) = (&commands[0], &commands[1]);

            assert_command(
                clone,
                &["clone", "--filter=blob:none", "--no-checkout", "--", URL, TARGET],
            );
            assert_command(checkout, &["-C", TARGET, "checkout", "--detach", rev, "--"]);
        }
    }

    #[test]
    fn git_commands_tag_and_branch_are_nonoverlapping() {
        // An identically named branch must not substitute for a requested tag.
        for (reference, qualified_name) in [
            (GitRef::Tag("release".into()), "refs/tags/release"),
            (GitRef::Branch("release".into()), "refs/heads/release"),
        ] {
            let commands = Install::git_commands(URL, Some(&reference), None, Path::new(TARGET));

            assert_eq!(commands.len(), 2);
            let (clone, checkout) = (&commands[0], &commands[1]);

            assert_command(
                clone,
                &[
                    "clone",
                    "--filter=blob:none",
                    "--no-checkout",
                    "--depth",
                    "1",
                    "--revision",
                    qualified_name,
                    "--",
                    URL,
                    TARGET,
                ],
            );
            assert_command(checkout, &["-C", TARGET, "checkout", "--detach", "HEAD", "--"]);
        }
    }

    #[test]
    fn git_commands_using_shallow_clone_by_default() {
        let commands = Install::git_commands(URL, None, None, Path::new(TARGET));

        assert_eq!(commands.len(), 2);
        let (clone, checkout) = (&commands[0], &commands[1]);

        assert_command(
            clone,
            &[
                "clone",
                "--filter=blob:none",
                "--no-checkout",
                "--depth",
                "1",
                "--",
                URL,
                TARGET,
            ],
        );
        assert_command(checkout, &["-C", TARGET, "checkout", "--detach", "HEAD", "--"]);
    }

    #[test]
    fn sparse_checkout_is_before_checking_out_the_revision() {
        let reference = GitRef::Rev(":/fix nasty bug".into());
        let commands = Install::git_commands(URL, Some(&reference), Some("libs/math"), Path::new(TARGET));

        assert_eq!(commands.len(), 3);
        let (clone, sparse_checkout, checkout) = (&commands[0], &commands[1], &commands[2]);

        assert_command(
            clone,
            &["clone", "--filter=blob:none", "--no-checkout", "--", URL, TARGET],
        );
        assert_command(
            sparse_checkout,
            &["-C", TARGET, "sparse-checkout", "set", "--no-cone", "/libs/math/"],
        );
        assert_command(
            checkout,
            &["-C", TARGET, "checkout", "--detach", ":/fix nasty bug", "--"],
        );
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn init_repo(dir: &Path, simplex_toml: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(smplx_build::CONFIG_FILENAME), simplex_toml).unwrap();

        git(dir, &["init", "--quiet"]);
        git(dir, &["add", "."]);
        git(dir, &["commit", "--quiet", "-m", "init"]);
    }

    #[test]
    fn failed_install_leaves_nothing_behind() {
        let root = std::env::temp_dir().join(format!("smplx-install-failed-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let deps_dir = root.join("deps");
        fs::create_dir_all(&deps_dir).unwrap();

        let url = format!("file://{}", root.join("missing").display());
        let deps =
            DependencyConfig::from_source(&format!("[dependencies]\nmissing = {{ git = \"{url}\" }}\n")).unwrap();

        assert!(Install::install_repos(&deps, &deps_dir, &mut Vec::new()).is_err());
        assert_eq!(
            fs::read_dir(&deps_dir).unwrap().count(),
            0,
            "neither a target nor a staging dir is left"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn incomplete_install_is_installed_again() {
        let root = std::env::temp_dir().join(format!("smplx-install-incomplete-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let url = format!("file://{}", root.join("a").display());
        init_repo(&root.join("a"), "");

        let deps = DependencyConfig::from_source(&format!("[dependencies]\na = {{ git = \"{url}\" }}\n")).unwrap();
        let deps_dir = root.join("deps");
        let target_dir = deps_dir.join(ArtifactsResolver::generate_hashed_repo_path(&url, None, None).unwrap());

        // An install killed after `git clone --no-checkout` left only the repository's `.git`
        fs::create_dir_all(target_dir.join(".git")).unwrap();

        let mut installed_repos = Vec::new();
        Install::install_repos(&deps, &deps_dir, &mut installed_repos).unwrap();

        assert_eq!(installed_repos, vec![target_dir.clone()]);
        assert!(target_dir.join(smplx_build::CONFIG_FILENAME).is_file());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cyclic_git_dependencies_terminate() {
        let root = std::env::temp_dir().join(format!("smplx-install-cycle-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let url_a = format!("file://{}", root.join("a").display());
        let url_b = format!("file://{}", root.join("b").display());

        // `a` depends on itself and on `b`, which depends back on `a`.
        init_repo(
            &root.join("a"),
            &format!("[dependencies]\na = {{ git = \"{url_a}\" }}\nb = {{ git = \"{url_b}\" }}\n"),
        );
        init_repo(
            &root.join("b"),
            &format!("[dependencies]\na = {{ git = \"{url_a}\" }}\n"),
        );

        let deps = DependencyConfig::from_source(&format!("[dependencies]\na = {{ git = \"{url_a}\" }}\n")).unwrap();
        let deps_dir = root.join("deps");
        fs::create_dir_all(&deps_dir).unwrap();

        let mut installed_repos = Vec::new();
        Install::install_repos(&deps, &deps_dir, &mut installed_repos).unwrap();

        assert_eq!(installed_repos.len(), 2);

        fs::remove_dir_all(root).unwrap();
    }
}
