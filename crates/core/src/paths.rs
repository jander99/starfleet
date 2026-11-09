//! Where Starfleet keeps its own files.
//!
//! Linux and macOS both follow the XDG layout (macOS on purpose, instead of
//! `~/Library`). Windows uses `%APPDATA%` and `%LOCALAPPDATA%`.
//! `STARFLEET_HOME` overrides everything, which is handy for demos and tests.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Result<Self> {
        if let Some(root) = non_empty_env("STARFLEET_HOME") {
            return Ok(Self::at(Path::new(&root)));
        }
        #[cfg(windows)]
        {
            let roaming = non_empty_env("APPDATA").ok_or_else(|| anyhow!("APPDATA is not set"))?;
            let local = non_empty_env("LOCALAPPDATA").unwrap_or_else(|| roaming.clone());
            Ok(Self {
                config_dir: PathBuf::from(roaming).join("starfleet"),
                data_dir: PathBuf::from(local).join("starfleet"),
            })
        }
        #[cfg(not(windows))]
        {
            let home = home_dir().ok_or_else(|| anyhow!("HOME is not set"))?;
            let config = non_empty_env("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"));
            let data = non_empty_env("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local").join("share"));
            Ok(Self {
                config_dir: config.join("starfleet"),
                data_dir: data.join("starfleet"),
            })
        }
    }

    /// Both directories under one root.
    pub fn at(root: &Path) -> Self {
        Self {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
        }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
    pub fn roster_file(&self) -> PathBuf {
        self.config_dir.join("roster.json")
    }
    pub fn board_file(&self) -> PathBuf {
        self.data_dir.join("board.json")
    }
}

pub fn home_dir() -> Option<PathBuf> {
    non_empty_env("HOME")
        .or_else(|| non_empty_env("USERPROFILE"))
        .map(PathBuf::from)
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

/// Per-machine settings, hand-edited in `settings.json`. All optional.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Explicit Herdr socket path. On Windows this is a path inside WSL.
    pub herdr_socket: Option<String>,
    /// Named Herdr session to use instead of the default one.
    pub herdr_session: Option<String>,
    /// WSL distribution to use on Windows. Default distribution when unset.
    pub wsl_distro: Option<String>,
}

impl Settings {
    pub fn load(paths: &Paths) -> Result<Self> {
        let file = paths.settings_file();
        match std::fs::read_to_string(&file) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| anyhow!("{} is not valid: {e}", file.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(anyhow!("cannot read {}: {e}", file.display())),
        }
    }
}

/// Write a file by writing a sibling temp file and renaming it over the target.
pub fn write_atomic(file: &Path, contents: &str) -> Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, file)?;
    Ok(())
}
