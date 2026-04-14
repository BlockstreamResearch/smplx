use fs3::FileExt;
use reqwest::blocking::Client;
use semver::Version;
use std::env;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use flate2::read::GzDecoder;
use tar::Archive;
use zip::ZipArchive;

use super::constants;
use crate::error::BuildError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformBinary {
    WindowsAmd,
    LinuxAmd,
    LinuxArm,
    MacosAmd,
    MacosArm,
}

impl PlatformBinary {
    /// Detects the current platform based on OS and Architecture
    pub fn current() -> Result<Self, BuildError> {
        match (env::consts::OS, env::consts::ARCH) {
            ("windows", "x86_64") => Ok(PlatformBinary::WindowsAmd),
            ("linux", "x86_64") => Ok(PlatformBinary::LinuxAmd),
            ("linux", "aarch64") => Ok(PlatformBinary::LinuxArm),
            ("macos", "x86_64") => Ok(PlatformBinary::MacosAmd),
            ("macos", "aarch64") => Ok(PlatformBinary::MacosArm),
            (os, arch) => Err(BuildError::VersionResolution(format!(
                "Unsupported OS/Arch combination: {}-{}",
                os, arch
            ))),
        }
    }

    /// Returns the expected executable filename
    pub fn filename(&self) -> &'static str {
        if matches!(self, PlatformBinary::WindowsAmd) {
            "simc.exe"
        } else {
            "simc"
        }
    }

    /// Formats the GitHub Release asset name (e.g., "simc-macos-aarch64.tar.gz")
    pub fn asset_name(&self) -> &'static str {
        match self {
            PlatformBinary::WindowsAmd => "simc-windows-x86_64.zip",
            PlatformBinary::LinuxAmd => "simc-linux-x86_64.tar.gz",
            PlatformBinary::LinuxArm => "simc-linux-aarch64.tar.gz",
            PlatformBinary::MacosAmd => "simc-macos-x86_64.tar.gz",
            PlatformBinary::MacosArm => "simc-macos-aarch64.tar.gz",
        }
    }
}

pub struct CompilerInfo {
    pub binary_path: PathBuf,
}

pub struct CompilerDownloader {
    platform: PlatformBinary,
    compilers_dir: PathBuf,
}

impl CompilerDownloader {
    pub fn new(compilers_dir: PathBuf) -> Self {
        Self {
            platform: PlatformBinary::current().expect("Unsupported platform"),
            compilers_dir,
        }
    }

    /// Checks if a compiler version is downloaded locally
    pub fn is_compiler_downloaded(&self, version: &str) -> bool {
        self.get_download_path(version).exists()
    }

    /// Retrieves the cached compiler binary path for execution
    pub fn get_compiler_binary(&self, version: &str) -> Result<CompilerInfo, BuildError> {
        let binary_path = self.get_download_path(version);
        if !binary_path.exists() {
            return Err(BuildError::VersionResolution(format!(
                "Compiler binary for version {} not found in cache at {}",
                version,
                binary_path.display()
            )));
        }
        Ok(CompilerInfo { binary_path })
    }

    /// Safely downloads the compiler using a multi-process lock
    pub fn download_compiler(&self, version: &str, verify: bool) -> Result<(), BuildError> {
        if self.is_compiler_downloaded(version) {
            return Ok(());
        }

        fs::create_dir_all(&self.compilers_dir)?;
        let lock_file_path = self.compilers_dir.join(constants::DOWNLOAD_LOCK_FILE);
        let lock_file = File::create(&lock_file_path)?;

        lock_file.lock_exclusive()?;

        if self.is_compiler_downloaded(version) {
            lock_file.unlock()?;
            return Ok(());
        }

        println!("--> Downloading compiler v{}...", version);

        let download_path = match self.perform_download(version) {
            Ok(path) => path,
            Err(e) => {
                lock_file.unlock()?;
                return Err(e);
            }
        };

        #[allow(clippy::collapsible_if)]
        if verify {
            if let Err(e) = self.post_process_compiler(&download_path) {
                let _ = fs::remove_file(&download_path);
                lock_file.unlock()?;
                return Err(e);
            }
        }

        lock_file.unlock()?;
        Ok(())
    }

    /// Returns a list of all compiler versions currently cached locally.
    pub fn get_downloaded_versions(&self) -> Vec<String> {
        let mut versions = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.compilers_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let dir_name = entry.file_name().into_string().unwrap_or_default();

                    let path = self.get_download_path(&dir_name);

                    if let Some(ver) = Version::parse(&dir_name).ok().filter(|_| path.exists()) {
                        versions.push(ver.to_string());
                    }
                }
            }
        }

        versions
    }

    fn perform_download(&self, version: &str) -> Result<PathBuf, BuildError> {
        let binary_path = self.get_download_path(version);
        let cache_dir = binary_path.parent().unwrap();

        fs::create_dir_all(cache_dir)?;

        let asset = self.platform.asset_name();

        let url = format!(
            "https://github.com/{}/{}/releases/download/simplicityhl-{}/{}",
            constants::REPO_OWNER,
            constants::REPO_NAME,
            version,
            asset
        );

        let client = Client::builder()
            .user_agent(constants::USER_AGENT)
            .build()
            .map_err(BuildError::Download)?;

        let response = client.get(&url).send().map_err(BuildError::Download)?;

        if !response.status().is_success() {
            return Err(BuildError::VersionResolution(format!(
                "Failed to download from GitHub. HTTP {}",
                response.status()
            )));
        }

        if asset.ends_with(".tar.gz") {
            let tar = GzDecoder::new(response);
            let mut archive = Archive::new(tar);
            archive.unpack(cache_dir)?;
        } else if asset.ends_with(".zip") {
            let zip_data = response.bytes().map_err(BuildError::Download)?;
            let reader = std::io::Cursor::new(zip_data);
            let mut archive = ZipArchive::new(reader)
                .map_err(|e| BuildError::Io(std::io::Error::other(format!("Failed to read zip archive: {}", e))))?;
            archive
                .extract(cache_dir)
                .map_err(|e| BuildError::Io(std::io::Error::other(format!("Failed to extract zip archive: {}", e))))?;
        } else {
            return Err(BuildError::VersionResolution(format!(
                "Unsupported archive format for asset: {}",
                asset
            )));
        }

        Ok(binary_path)
    }

    fn post_process_compiler(&self, path: &Path) -> Result<(), BuildError> {
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(path)
                .map_err(|e| {
                    BuildError::VersionResolution(format!("File missing or inaccessible at {:?}: {}", path, e))
                })?
                .permissions();

            perms.set_mode(0o755);
            fs::set_permissions(path, perms)
                .map_err(|e| BuildError::VersionResolution(format!("Failed to set executable permissions: {}", e)))?;
        }

        let output = Command::new(path)
            .arg("--help")
            .output()
            .map_err(|e| BuildError::VersionResolution(format!("OS refused to execute binary {:?}: {}", path, e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            return Err(BuildError::VersionResolution(format!(
                "Compiler executed but crashed! \nExit code: {:?}\nStdout: {}\nStderr: {}",
                output.status.code(),
                stdout,
                stderr
            )));
        }

        Ok(())
    }

    fn get_download_path(&self, version: &str) -> PathBuf {
        self.compilers_dir.join(version).join(self.platform.filename())
    }
}
