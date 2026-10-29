use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use smplx_build::{ArtifactsResolver, BuildConfig};

use crate::commands::FormatOpts;
use crate::commands::error::{CommandError, FmtError};
use crate::config::Config;

const SIMFMT_BIN_NAME: &str = "simfmt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verbosity {
    Verbose,
    Normal,
    Quiet,
}

pub struct Format;

impl Format {
    /// Formats every configured `.simf` source file in one `simfmt` process.
    ///
    /// Explicit files and informational requests do not require a config.
    ///
    /// # Errors
    /// Returns a [`CommandError`] if the options or project configuration are
    /// invalid, source discovery fails, no source files match, or `simfmt` cannot
    /// be executed.
    pub fn run(opts: &FormatOpts) -> Result<(), CommandError> {
        let verbosity = Self::verbosity(opts)?;

        let files = if Self::is_info_request(opts) {
            Vec::default()
        } else if opts.files.is_empty() {
            let config_path = Config::get_ancestors_config_path().map_err(FmtError::from)?;
            let project_root = Config::get_project_root(&config_path).map_err(FmtError::from)?;
            let loaded_config = Config::load(&config_path).map_err(FmtError::from)?;

            Self::resolve_files(&loaded_config.build, project_root)?
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            opts.files
                .iter()
                .map(|path| path.canonicalize().unwrap_or_else(|_| path.clone()))
                .collect::<Vec<_>>()
        };

        let args = Self::build_simfmt_args(opts, &files);

        let signal = Self::run_simfmt(&args, verbosity)?;

        Self::flush(signal)
    }

    fn is_info_request(opts: &FormatOpts) -> bool {
        opts.simfmt_options.iter().any(|arg| {
            ["--print-config", "-h", "--help", "-V", "--version"].contains(&arg.as_str())
                || arg.starts_with("--help=")
                || arg.starts_with("--print-config=")
        })
    }

    fn resolve_files(config: &BuildConfig, project_root: impl AsRef<Path>) -> Result<BTreeSet<PathBuf>, CommandError> {
        let src_dir = project_root.as_ref().join(&config.src_dir);
        let files = ArtifactsResolver::resolve_simf_files(project_root, &config.src_dir, &config.simf_files)?;

        if files.is_empty() {
            return Err(FmtError::NoFiles(src_dir).into());
        }

        Ok(files)
    }

    fn verbosity(opts: &FormatOpts) -> Result<Verbosity, FmtError> {
        match (opts.verbose, opts.quiet) {
            (false, false) => Ok(Verbosity::Normal),
            (false, true) => Ok(Verbosity::Quiet),
            (true, false) => Ok(Verbosity::Verbose),
            (true, true) => Err(FmtError::ConflictingVerbosity),
        }
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

    /// Flushes the STDOUT buffer and exits with the formatter's exit status.
    fn flush(status: i32) -> Result<(), CommandError> {
        use std::io::Write;

        // Flush buffered standard output before exiting.
        std::io::stdout().flush()?;

        // Exit with the formatter's success or failure status.
        std::process::exit(status);
    }

    /// Resolves the file path to the `simfmt` binary in the same directory where `simplex` is situated.
    /// Given that `simfmt` would be placed in one directory with `simplex`, this is suitable for our usecase.
    ///
    /// # Warning
    /// Can fail to search for binary when env isn't set and `simfmt` binary lies in the other directory.
    /// Even when `simfmt` is visible through PATH.
    #[inline]
    fn simfmt_binary() -> Result<PathBuf, FmtError> {
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

    use super::{Format, FormatOpts};

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

        assert_eq!(files.len(), 17);
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
