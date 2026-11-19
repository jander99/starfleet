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
    /// Per-card logs live beside the board, one JSONL file each.
    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }
    pub fn log_file(&self, card_id: &str) -> PathBuf {
        self.logs_dir().join(format!("{card_id}.jsonl"))
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
    use std::io::Write;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("tmp");
    // fsync before the rename: otherwise a power loss can leave the rename
    // durable while the written bytes are not, which is an empty board file.
    let mut out = std::fs::File::create(&tmp)?;
    out.write_all(contents.as_bytes())?;
    out.sync_all()?;
    drop(out);
    std::fs::rename(&tmp, file)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use crate::test_support::{env_guard, EnvRestore};

    fn root() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn at_puts_config_and_data_under_one_root() {
        let p = Paths::at(Path::new("/srv/sf"));
        assert_eq!(p.config_dir, Path::new("/srv/sf/config"));
        assert_eq!(p.data_dir, Path::new("/srv/sf/data"));
        assert_eq!(p.settings_file(), Path::new("/srv/sf/config/settings.json"));
        assert_eq!(p.roster_file(), Path::new("/srv/sf/config/roster.json"));
        assert_eq!(p.board_file(), Path::new("/srv/sf/data/board.json"));
        assert_eq!(p.logs_dir(), Path::new("/srv/sf/data/logs"));
        assert_eq!(p.log_file("c1"), Path::new("/srv/sf/data/logs/c1.jsonl"));
    }

    #[test]
    fn resolve_honors_starfleet_home() {
        // Serialized with the other env tests through a guard.
        let _g = env_guard();
        let _restore = EnvRestore::new(&["STARFLEET_HOME"]);
        std::env::set_var("STARFLEET_HOME", "/tmp/sf-home");
        let p = Paths::resolve().unwrap();
        assert_eq!(p.config_dir, Path::new("/tmp/sf-home/config"));
        assert_eq!(p.data_dir, Path::new("/tmp/sf-home/data"));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_uses_xdg_then_dotfiles() {
        let _g = env_guard();
        let _restore =
            EnvRestore::new(&["STARFLEET_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "HOME"]);
        for k in ["STARFLEET_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "HOME"] {
            std::env::remove_var(k);
        }
        std::env::set_var("HOME", "/home/me");
        let p = Paths::resolve().unwrap();
        assert_eq!(p.config_dir, Path::new("/home/me/.config/starfleet"));
        assert_eq!(p.data_dir, Path::new("/home/me/.local/share/starfleet"));

        std::env::set_var("XDG_CONFIG_HOME", "/cfg");
        std::env::set_var("XDG_DATA_HOME", "/dat");
        let p = Paths::resolve().unwrap();
        assert_eq!(p.config_dir, Path::new("/cfg/starfleet"));
        assert_eq!(p.data_dir, Path::new("/dat/starfleet"));

        // A blank value counts as unset.
        std::env::set_var("XDG_CONFIG_HOME", "   ");
        let p = Paths::resolve().unwrap();
        assert_eq!(p.config_dir, Path::new("/home/me/.config/starfleet"));
    }

    #[cfg(unix)]
    #[test]
    fn home_dir_falls_back_to_userprofile() {
        let _g = env_guard();
        let _restore = EnvRestore::new(&["HOME", "USERPROFILE"]);
        std::env::remove_var("HOME");
        std::env::set_var("USERPROFILE", "/users/me");
        assert_eq!(home_dir(), Some(PathBuf::from("/users/me")));
    }

    #[test]
    fn settings_default_when_missing_and_error_when_bad() {
        let dir = root();
        let paths = Paths::at(dir.path());
        let s = Settings::load(&paths).unwrap();
        assert!(s.herdr_socket.is_none() && s.herdr_session.is_none() && s.wsl_distro.is_none());

        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(paths.settings_file(), "{ not json").unwrap();
        let err = Settings::load(&paths).unwrap_err().to_string();
        assert!(err.contains("is not valid"), "{err}");

        std::fs::write(paths.settings_file(), r#"{"herdr_session":"work"}"#).unwrap();
        let s = Settings::load(&paths).unwrap();
        assert_eq!(s.herdr_session.as_deref(), Some("work"));
    }

    #[test]
    fn write_atomic_creates_dirs_and_replaces() {
        let dir = root();
        let file = dir.path().join("deep/nested/file.txt");
        write_atomic(&file, "one").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "one");
        write_atomic(&file, "two").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
        // The temp sibling is gone after the rename.
        assert!(!file.with_extension("tmp").exists());
    }

    #[test]
    fn write_atomic_errors_on_an_unwritable_target() {
        let dir = root();
        // A directory where the file should go makes the rename fail.
        let target = dir.path().join("d");
        std::fs::create_dir(&target).unwrap();
        assert!(write_atomic(&target, "x").is_err());
    }
}
