use std::collections::BTreeSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use smplx_build::{ArtifactsResolver, BuildConfig};

use crate::commands::FormatOpts;
use crate::commands::error::{CommandError, FmtError};
use crate::config::CONFIG_FILENAME;

const SIMFMT_BIN_NAME: &str = "simfmt";
const SIMFMT_BIN_PATH_VAR_ENV: &str = "SIMFMT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verbosity {
    Verbose,
    Normal,
    Quiet,
}

pub struct Format;

impl Format {
    /// Returns whether the request only asks `simfmt` for information and does not
    /// need a Simplex project to be loaded first.
    #[must_use]
    pub fn is_info_request(opts: &FormatOpts) -> bool {
        opts.simfmt_options.iter().any(|arg| {
            ["--print-config", "-h", "--help", "-V"].contains(&arg.as_str())
                || arg.starts_with("--help=")
                || arg.starts_with("--print-config=")
        })
    }

    /// Resolves the selected manifest to an absolute path.
    ///
    /// # Errors
    /// Returns a [`FmtError`] when the current directory cannot be read or the
    /// supplied path does not name a `Simplex.toml` file.
    pub fn manifest_path() -> Result<PathBuf, FmtError> {
        let current_dir = env::current_dir().map_err(FmtError::CurrentDir)?;
        Self::resolve_manifest_path(&current_dir)
    }

    fn resolve_manifest_path(current_dir: impl AsRef<Path>) -> Result<PathBuf, FmtError> {
        let current_dir = current_dir.as_ref();
        let manifest_path = {
            let path = current_dir
                .ancestors()
                .map(|directory| directory.join(CONFIG_FILENAME))
                .find(|candidate| candidate.is_file());

            if path.is_none() {
                return Err(FmtError::FailedToFindManifest);
            }

            path.unwrap()
        };

        if manifest_path.file_name() != Some(OsStr::new(CONFIG_FILENAME)) {
            return Err(FmtError::InvalidManifestPath(manifest_path));
        }

        if manifest_path.is_absolute() {
            Ok(manifest_path)
        } else {
            Ok(current_dir.join(manifest_path))
        }
    }

    /// Runs an informational `simfmt` request such as `--help`,
    /// without loading a Simplex manifest.
    ///
    /// # Errors
    /// Returns a [`CommandError`] if the options conflict or `simfmt` cannot be
    /// executed.
    pub fn run_info(opts: &FormatOpts) -> Result<i32, CommandError> {
        let verbosity = Self::verbosity(opts)?;
        let args = opts.simfmt_options.iter().map(OsString::from).collect::<Vec<_>>();

        Ok(Self::run_simfmt(&args, verbosity)?)
    }

    /// Formats every configured `.simf` source file in one `simfmt` process.
    ///
    /// # Errors
    /// Returns a [`CommandError`] if the options are invalid, source discovery
    /// fails, no source files match, or `simfmt` cannot be executed.
    pub fn run(opts: &FormatOpts, files: &[PathBuf]) -> Result<i32, CommandError> {
        let verbosity = Self::verbosity(opts)?;
        let args = Self::build_simfmt_args(opts, files);

        Ok(Self::run_simfmt(&args, verbosity)?)
    }

    fn verbosity(opts: &FormatOpts) -> Result<Verbosity, FmtError> {
        match (opts.verbose, opts.quiet) {
            (false, false) => Ok(Verbosity::Normal),
            (false, true) => Ok(Verbosity::Quiet),
            (true, false) => Ok(Verbosity::Verbose),
            (true, true) => Err(FmtError::ConflictingVerbosity),
        }
    }

    pub(crate) fn resolve_files(
        config: &BuildConfig,
        project_root: impl AsRef<Path>,
    ) -> Result<BTreeSet<PathBuf>, CommandError> {
        let src_dir = project_root.as_ref().join(&config.src_dir);
        let files = ArtifactsResolver::resolve_simf_files(project_root, &config.src_dir, &config.simf_files)?;

        if files.is_empty() {
            return Err(FmtError::NoFiles(src_dir).into());
        }

        Ok(files)
    }

    fn build_simfmt_args(opts: &FormatOpts, files: &[PathBuf]) -> Vec<OsString> {
        let mut simfmt_args = Vec::with_capacity(opts.simfmt_options.len() + 3);

        if opts.quiet {
            simfmt_args.push(OsString::from("--quiet"));
        }
        if opts.verbose {
            simfmt_args.push(OsString::from("--verbose"));
        }
        if opts.check && !opts.simfmt_options.iter().any(|arg| arg == "--check") {
            simfmt_args.push(OsString::from("--check"));
        }
        simfmt_args.extend(opts.simfmt_options.iter().map(OsString::from));

        if opts.short
            && !opts
                .simfmt_options
                .iter()
                .any(|arg| arg == "-l" || arg == "--files-with-diff")
        {
            simfmt_args.push(OsString::from("-l"));
        }

        let mut args = Vec::with_capacity(files.len() + simfmt_args.len());

        for file in files {
            args.push(file.as_os_str().to_owned());
        }

        args.extend(simfmt_args);
        args
    }

    fn run_simfmt(args: &[OsString], verbosity: Verbosity) -> Result<i32, FmtError> {
        let binary = Self::simfmt_binary()?;
        let mut command = Command::new(&binary);
        command
            .args(args)
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdout(if verbosity == Verbosity::Quiet {
                Stdio::null()
            } else {
                Stdio::inherit()
            });

        if verbosity == Verbosity::Verbose {
            println!("{command:?}");
        }

        let status = command
            .status()
            .map_err(|source| FmtError::RunSimfmt { binary, source })?;

        Ok(Self::normalize_status(status))
    }

    /// Resolves the file path to the `simfmt` binary in the same folder where `simplex` is situated.
    /// Given that `simfmt` would be placed in one directory with `simplex`, this is suitable for our usecase.
    ///
    /// Path to the binary can be overrided with `SIMFMT_BIN_PATH_VAR_ENV`.
    ///
    /// # Warning
    /// Can fail to search for binary when env isn't set and `simfmt` binary lies in the other directory.
    /// Even when `simfmt` is visible through PATH
    fn simfmt_binary() -> Result<PathBuf, FmtError> {
        if let Some(simfmt) = env::var_os(SIMFMT_BIN_PATH_VAR_ENV) {
            return Ok(PathBuf::from(simfmt));
        }

        Ok(env::current_exe()
            .map_err(FmtError::CurrentExecutable)?
            .with_file_name(SIMFMT_BIN_NAME))
    }

    fn normalize_status(status: ExitStatus) -> i32 {
        const SUCCESS: i32 = 0;
        const FAILURE: i32 = 1;

        if status.success() { SUCCESS } else { FAILURE }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};

    use clap::Parser;
    use smplx_build::BuildConfig;

    use crate::Cli;
    use crate::commands::Command as SimplexCommand;

    use super::{CONFIG_FILENAME, Format, FormatOpts};

    fn fmt_opts(args: impl IntoIterator<Item = &'static str>) -> FormatOpts {
        let cli = Cli::try_parse_from(args).expect("arguments should parse");
        match cli.command {
            SimplexCommand::Fmt { opts } => opts,
            _ => panic!("expected fmt command"),
        }
    }

    #[test]
    fn parses_default_options() {
        let opts = fmt_opts(["simplex", "fmt"]);

        assert!(!opts.quiet);
        assert!(!opts.verbose);
        assert!(!opts.check);
        assert!(!opts.short);
        assert_eq!(opts.simfmt_options, [] as [std::string::String; 0]);
    }

    #[test]
    fn parses_all_options_and_raw_simfmt_arguments() {
        let opts = fmt_opts([
            "simplex", "fmt", "--quiet", "--short", "--check", "--", "--emit", "stdout",
        ]);

        assert!(opts.quiet);
        assert!(!opts.verbose);
        assert!(opts.check);
        assert!(opts.short);
        assert_eq!(opts.simfmt_options, ["--emit", "stdout"]);
    }

    #[test]
    fn raw_simfmt_arguments_require_separator() {
        assert!(Cli::try_parse_from(["simplex", "fmt", "--emit", "stdout"]).is_err());
        assert!(Cli::try_parse_from(["simplex", "fmt", "--", "--emit", "stdout"]).is_ok());

        assert!(Cli::try_parse_from(["simplex", "fmt", "--color", "auto"]).is_err());
        assert!(Cli::try_parse_from(["simplex", "fmt", "--", "--color", "auto"]).is_ok());
    }

    #[test]
    fn rejects_unknown_wrapper_arguments() {
        assert!(Cli::try_parse_from(["simplex", "fmt", "--package", "demo"]).is_err());
        assert!(Cli::try_parse_from(["simplex", "fmt", "--all"]).is_err());
    }

    #[test]
    fn discovers_manifest_in_current_directory() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");

        let manifest_path = Format::resolve_manifest_path(&project_root).expect("manifest should be discovered");

        assert_eq!(manifest_path, project_root.join(CONFIG_FILENAME));
    }

    #[test]
    fn discovers_manifest_from_nested_directory() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let nested_directory = project_root.join("simf/nested/nested_2/imp2");

        let manifest_path = Format::resolve_manifest_path(&nested_directory).expect("manifest should be discovered");

        assert_eq!(manifest_path, project_root.join(CONFIG_FILENAME));
    }

    #[test]
    fn discovers_nearest_manifest() {
        let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let project_root = fixtures_root.join("deps/math");
        let nested_directory = project_root.join("simf");

        let manifest_path = Format::resolve_manifest_path(&nested_directory).expect("manifest should be discovered");

        assert_eq!(manifest_path, project_root.join(CONFIG_FILENAME));
    }

    #[test]
    fn builds_deterministic_simfmt_arguments() {
        let opts = fmt_opts(["simplex", "fmt", "--verbose", "--check", "--", "--emit", "files"]);
        let files = [PathBuf::from("/tmp/a.simf"), PathBuf::from("/tmp/path/spaces/b.simf")];

        let args = Format::build_simfmt_args(&opts, &files);

        assert_eq!(
            args,
            [
                OsString::from("/tmp/a.simf"),
                OsString::from("/tmp/path/spaces/b.simf"),
                OsString::from("--verbose"),
                OsString::from("--check"),
                OsString::from("--emit"),
                OsString::from("files"),
            ]
        );
    }

    #[test]
    fn builds_deterministic_simfmt_arguments_with_files() {
        let opts = fmt_opts([
            "simplex",
            "fmt",
            "/tmp/a.simf",
            "--verbose",
            "--check",
            "/tmp/b.simf",
            "/tmp/path/spaces/b.simf",
            "--",
            "--emit",
            "files",
        ]);

        let args = Format::build_simfmt_args(&opts, &opts.files);

        assert_eq!(
            args,
            [
                OsString::from("/tmp/a.simf"),
                OsString::from("/tmp/b.simf"),
                OsString::from("/tmp/path/spaces/b.simf"),
                OsString::from("--verbose"),
                OsString::from("--check"),
                OsString::from("--emit"),
                OsString::from("files"),
            ]
        );
    }

    #[test]
    fn converts_short_message_format_to_list_files() {
        let opts = fmt_opts(["simplex", "fmt", "--short"]);
        let args = Format::build_simfmt_args(&opts, &["/tmp/a.simf".into()]);

        assert_eq!(args.last(), Some(&OsString::from("-l")));
    }

    #[test]
    fn does_not_duplicate_existing_list_files_flag() {
        let opts = fmt_opts(["simplex", "fmt", "--short", "--", "--files-with-diff"]);
        let args = Format::build_simfmt_args(&opts, &["/tmp/a.simf".into()]);

        assert_eq!(args.iter().filter(|arg| *arg == "--files-with-diff").count(), 1);
        assert!(!args.iter().any(|arg| arg == "-l"));
    }

    #[test]
    fn does_not_duplicate_raw_check_flag() {
        let opts = fmt_opts(["simplex", "fmt", "--check", "--", "--check"]);
        let args = Format::build_simfmt_args(&opts, &["/tmp/a.simf".into()]);

        assert_eq!(args.iter().filter(|arg| *arg == "--check").count(), 1);
    }

    #[test]
    fn resolves_all_configured_files_in_sorted_order() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let config = BuildConfig {
            simf_files: vec!["*.simf".into(), "**/*.simf".into()],
            src_dir: "simf".into(),
            out_dir: "None".into(),
        };

        let files = Format::resolve_files(&config, &project_root).expect("fixture files should resolve");

        assert_eq!(files.len(), 6);
        assert!(files.iter().all(|path| path.extension() == Some(OsStr::new("simf"))));
    }

    #[test]
    fn rejects_an_empty_formatter_workspace() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let config = BuildConfig {
            simf_files: vec!["no-such-file-*.simf".into()],
            src_dir: "simf".into(),
            out_dir: "None".into(),
        };

        assert!(Format::resolve_files(&config, &project_root).is_err());
    }
}
