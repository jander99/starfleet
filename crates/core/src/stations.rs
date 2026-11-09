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
    pub agent: String,
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
