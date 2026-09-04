//! Keep the installed binary current: recognise how it got here, ask GitHub
//! for the newest release, and run the matching upgrade path.

use std::cmp::Ordering;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

const REPO: &str = "MortenHusted/lemmaspec";
const FORMULA: &str = "MortenHusted/tap/lemmaspec";

/// How this binary was installed, which decides how it is upgraded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Install {
    /// `brew install MortenHusted/tap/lemmaspec`.
    Homebrew,
    /// The cargo-dist shell or PowerShell installer, which leaves a receipt.
    Installer { receipt: PathBuf },
    /// `cargo install --path .` or `cargo install lemmaspec`.
    Cargo,
    /// A build we cannot place: a repository target directory or a copied binary.
    Unknown,
}

impl Install {
    pub fn detect() -> Install {
        let exe = std::env::current_exe()
            .and_then(|path| path.canonicalize())
            .unwrap_or_default();
        Self::detect_from(&exe, receipt_path().filter(|path| path.is_file()))
    }

    /// Pure classification so the rules can be tested without a filesystem.
    pub fn detect_from(exe: &Path, receipt: Option<PathBuf>) -> Install {
        let path = exe.to_string_lossy().replace('\\', "/");
        if path.contains("/Cellar/lemmaspec/")
            || path.contains("/homebrew/")
            || path.contains("/linuxbrew/")
        {
            return Install::Homebrew;
        }
        // The installer and `cargo install` both land in Cargo's bin dir;
        // only the installer leaves a receipt. A receipt on the machine says
        // nothing about a binary running from somewhere else.
        if path.contains("/.cargo/bin/") {
            return match receipt {
                Some(receipt) => Install::Installer { receipt },
                None => Install::Cargo,
            };
        }
        Install::Unknown
    }

    /// The command that upgrades this kind of install, if one can be run.
    pub fn upgrade_command(&self) -> Option<Vec<String>> {
        match self {
            Install::Homebrew => Some(vec!["brew".into(), "upgrade".into(), FORMULA.into()]),
            Install::Installer { .. } if cfg!(windows) => Some(vec![
                "powershell".into(),
                "-NoProfile".into(),
                "-Command".into(),
                format!("irm https://github.com/{REPO}/releases/latest/download/lemmaspec-installer.ps1 | iex"),
            ]),
            Install::Installer { .. } => Some(vec![
                "sh".into(),
                "-c".into(),
                format!("curl --proto '=https' --tlsv1.2 -LsSf https://github.com/{REPO}/releases/latest/download/lemmaspec-installer.sh | sh"),
            ]),
            Install::Cargo | Install::Unknown => None,
        }
    }

    /// What to do by hand when no command applies.
    pub fn advice(&self) -> &'static str {
        match self {
            Install::Cargo => "installed with cargo: run `git pull && cargo install --path .` in your checkout, or `cargo install lemmaspec --locked` if you installed from crates.io",
            Install::Unknown => "this binary was not installed by Homebrew, the installer, or cargo; replace it the way you obtained it",
            Install::Homebrew | Install::Installer { .. } => "",
        }
    }
}

impl fmt::Display for Install {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Install::Homebrew => formatter.write_str("Homebrew"),
            Install::Installer { receipt } => {
                write!(formatter, "release installer ({})", receipt.display())
            }
            Install::Cargo => formatter.write_str("cargo install"),
            Install::Unknown => formatter.write_str("unknown"),
        }
    }
}

fn receipt_path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };
    base.map(|base| base.join("lemmaspec").join("lemmaspec-receipt.json"))
}

/// The newest published version, asked of GitHub through curl so the crate
/// stays free of network dependencies. Returns the bare version, no `v`.
pub fn latest_version() -> Result<String, String> {
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "15",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: lemmaspec",
            &format!("https://api.github.com/repos/{REPO}/releases/latest"),
        ])
        .output()
        .map_err(|error| format!("could not run curl to check for releases: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "could not reach GitHub releases: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let release: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("unexpected release metadata: {error}"))?;
    let tag = release["tag_name"]
        .as_str()
        .ok_or("release metadata has no tag_name")?;
    Ok(tag.trim_start_matches('v').to_string())
}

/// Compare two `major.minor.patch` versions; anything unparsable sorts first.
pub fn compare_versions(current: &str, latest: &str) -> Ordering {
    parse(current).cmp(&parse(latest))
}

fn parse(version: &str) -> (u64, u64, u64) {
    let mut parts = version
        .trim_start_matches('v')
        .split(['.', '-', '+'])
        .map(|part| part.parse::<u64>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_installs_by_where_the_binary_lives() {
        assert_eq!(
            Install::detect_from(
                Path::new("/opt/homebrew/Cellar/lemmaspec/0.1.0/bin/lemmaspec"),
                None
            ),
            Install::Homebrew
        );
        assert_eq!(
            Install::detect_from(Path::new("/home/me/.cargo/bin/lemmaspec"), None),
            Install::Cargo
        );
        let receipt = PathBuf::from("/home/me/.config/lemmaspec/lemmaspec-receipt.json");
        assert_eq!(
            Install::detect_from(
                Path::new("/home/me/.cargo/bin/lemmaspec"),
                Some(receipt.clone())
            ),
            Install::Installer {
                receipt: receipt.clone()
            }
        );
        assert_eq!(
            Install::detect_from(
                Path::new("/src/lemmaspec/target/debug/lemmaspec"),
                Some(receipt.clone())
            ),
            Install::Unknown,
            "a receipt elsewhere does not claim a repository build"
        );
    }

    #[test]
    fn orders_versions_numerically() {
        assert_eq!(compare_versions("0.1.0", "v0.2.0"), Ordering::Less);
        assert_eq!(compare_versions("0.10.0", "0.9.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Ordering::Equal);
    }
}
