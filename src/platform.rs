use std::fmt;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::ValueEnum;

/// Brave browser release channel.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Channel {
    /// Brave stable release
    #[default]
    Release,
    /// Brave Beta
    Beta,
    /// Brave Nightly
    Nightly,
    /// Brave Development
    Dev,
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Channel::Release => write!(f, "Release"),
            Channel::Beta => write!(f, "Beta"),
            Channel::Nightly => write!(f, "Nightly"),
            Channel::Dev => write!(f, "Development"),
        }
    }
}

impl Channel {
    /// Directory name suffix under BraveSoftware/ for each channel.
    fn dir_name(self) -> &'static str {
        match self {
            Channel::Release => "Brave-Browser",
            Channel::Beta => "Brave-Browser-Beta",
            Channel::Nightly => "Brave-Browser-Nightly",
            Channel::Dev => "Brave-Browser-Development",
        }
    }

}

/// Return the Brave Browser data directory for a given channel.
pub fn brave_dir_for_channel(channel: Channel) -> Result<PathBuf> {
    let home = home_dir()?;

    #[cfg(target_os = "macos")]
    {
        Ok(home.join("Library/Application Support/BraveSoftware").join(channel.dir_name()))
    }

    #[cfg(target_os = "linux")]
    {
        Ok(home.join(".config/BraveSoftware").join(channel.dir_name()))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        anyhow::bail!("unsupported platform; use --brave-dir to specify the path manually")
    }
}

/// Resolve the Brave data directory from --brave-dir override, --channel, or default.
///
/// Precedence: --brave-dir > --channel > default (Release).
/// Prints the resolved directory to stderr so the user knows which browser is targeted.
pub fn resolve_brave_dir(override_dir: Option<&str>, channel: Channel) -> Result<PathBuf> {
    match override_dir {
        Some(path) => {
            let p = PathBuf::from(path);
            if !p.is_dir() {
                anyhow::bail!("specified --brave-dir does not exist: {}", p.display());
            }
            eprintln!("Using custom Brave directory: {}", p.display());
            Ok(p)
        }
        None => {
            let p = brave_dir_for_channel(channel)?;
            if !p.is_dir() {
                anyhow::bail!(
                    "Brave {} data directory not found at {}. Is Brave {} installed?",
                    channel,
                    p.display(),
                    channel,
                );
            }
            eprintln!("Using Brave {} ({})", channel, p.display());
            Ok(p)
        }
    }
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("could not determine home directory")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_dir_names() {
        if cfg!(target_os = "macos") {
            let release = brave_dir_for_channel(Channel::Release).unwrap();
            let beta = brave_dir_for_channel(Channel::Beta).unwrap();
            let nightly = brave_dir_for_channel(Channel::Nightly).unwrap();
            let dev = brave_dir_for_channel(Channel::Dev).unwrap();
            assert!(release.ends_with("Brave-Browser"));
            assert!(beta.ends_with("Brave-Browser-Beta"));
            assert!(nightly.ends_with("Brave-Browser-Nightly"));
            assert!(dev.ends_with("Brave-Browser-Development"));
        }
    }

    #[test]
    fn test_override_brave_dir() {
        let tmp = std::env::temp_dir();
        let result = resolve_brave_dir(Some(tmp.to_str().unwrap()), Channel::Release);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), tmp);
    }

    #[test]
    fn test_override_brave_dir_nonexistent() {
        let result = resolve_brave_dir(Some("/nonexistent/path/12345"), Channel::Release);
        assert!(result.is_err());
    }
}
