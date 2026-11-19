//! The roster: locations, and the stations inside them.
//!
//! A station is a preset: which agent CLI to start, an optional model, extra
//! arguments and a base prompt. The roster lives in `roster.json` in the
//! config directory and is meant to be edited by hand; it is seeded from the
//! built-in default the first time the app runs.

use crate::paths::{write_atomic, Paths};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_ROSTER: &str = include_str!("default_roster.json");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Station {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub post: String,
    #[serde(default)]
    pub duty: String,
    /// Herdr agent kind: "claude", "opencode", "codex", ...
    /// Serialized as `harness`; `agent` is still accepted when reading.
    #[serde(alias = "agent")]
    pub harness: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub prompt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Location {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub subtitle: String,
    pub stations: Vec<Station>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Roster {
    pub locations: Vec<Location>,
}

impl Roster {
    pub fn builtin() -> Self {
        serde_json::from_str(DEFAULT_ROSTER).expect("built-in roster is valid")
    }

    pub fn load_or_seed(paths: &Paths) -> Result<Self> {
        let file = paths.roster_file();
        match std::fs::read_to_string(&file) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| anyhow!("{} is not a valid roster: {e}", file.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                write_atomic(&file, DEFAULT_ROSTER)?;
                Ok(Self::builtin())
            }
            Err(e) => Err(anyhow!("cannot read {}: {e}", file.display())),
        }
    }

    pub fn station(&self, id: &str) -> Option<&Station> {
        self.locations
            .iter()
            .flat_map(|l| l.stations.iter())
            .find(|s| s.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn station_json(harness_key: &str) -> String {
        format!(
            r#"{{"id":"s","name":"Data","post":"","duty":"",{harness_key}:"claude","model":null,"args":[],"prompt":""}}"#
        )
    }

    #[test]
    fn the_old_agent_key_still_parses() {
        let st: Station = serde_json::from_str(&station_json("\"agent\"")).unwrap();
        assert_eq!(st.harness, "claude");
        let out = serde_json::to_string(&st).unwrap();
        assert!(out.contains("\"harness\""));
        assert!(!out.contains("\"agent\""));
    }

    #[test]
    fn the_new_harness_key_parses_too() {
        let st: Station = serde_json::from_str(&station_json("\"harness\"")).unwrap();
        assert_eq!(st.harness, "claude");
    }

    #[test]
    fn builtin_roster_has_unique_station_ids() {
        let roster = Roster::builtin();
        let ids: Vec<_> = roster
            .locations
            .iter()
            .flat_map(|l| &l.stations)
            .map(|s| &s.id)
            .collect();
        assert_eq!(ids.len(), ids.iter().collect::<HashSet<_>>().len());
        assert!(roster.station("daystrom.research").is_some());
    }
}

    #[test]
    fn a_bad_roster_file_is_an_error_and_an_unreadable_one_too() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::at(dir.path());
        std::fs::create_dir_all(&paths.config_dir).unwrap();

        std::fs::write(paths.roster_file(), "{ nope").unwrap();
        let err = Roster::load_or_seed(&paths).unwrap_err().to_string();
        assert!(err.contains("is not a valid roster"), "{err}");

        // A directory where the file should be is not "not found", so it fails.
        std::fs::remove_file(paths.roster_file()).unwrap();
        std::fs::create_dir(paths.roster_file()).unwrap();
        let err = Roster::load_or_seed(&paths).unwrap_err().to_string();
        assert!(err.contains("cannot read"), "{err}");
    }

    #[test]
    fn a_missing_roster_is_seeded_from_the_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::at(dir.path());
        let seeded = Roster::load_or_seed(&paths).unwrap();
        assert!(paths.roster_file().exists());
        assert_eq!(seeded.locations.len(), Roster::builtin().locations.len());
        // The written file reads back identically.
        let again = Roster::load_or_seed(&paths).unwrap();
        assert_eq!(again.locations.len(), seeded.locations.len());
    }
