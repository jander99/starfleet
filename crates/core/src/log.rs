//! The Captain's Log and personal logs: append-only JSONL per card.
//!
//! The board stays the index; the log is the narrative. Entries are never
//! edited or removed, so a crash can only cost the last line — and `read`
//! tolerates that.

use crate::paths::Paths;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;

/// One line of a card's log. `owner` is "captain" or an agent id; `session_id`
/// points back into the harness's own transcript instead of copying it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub owner: String,
    pub at: u64,
    /// "prompt" | "result" | "artifact" | "decision" | "note"
    pub kind: String,
    pub text: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// The log file for a card id, refusing one that would escape the logs
/// directory — the id becomes a filename, so read and write share the check.
fn log_file_for(paths: &Paths, card_id: &str) -> Result<std::path::PathBuf> {
    if card_id.is_empty() || card_id.contains(['/', '\\']) || card_id.contains("..") {
        bail!("not a usable card id for a log file: {card_id:?}");
    }
    Ok(paths.log_file(card_id))
}

/// Append one entry to `<data_dir>/logs/<card_id>.jsonl`.
///
/// A crash can leave a torn last line with no trailing newline; appending onto
/// it would glue the two together and lose the new entry on read. So a leading
/// `\n` is written whenever the file does not already end in one.
pub fn append(paths: &Paths, card_id: &str, entry: &LogEntry) -> Result<()> {
    let file = log_file_for(paths, card_id)?;
    std::fs::create_dir_all(paths.logs_dir())?;
    let needs_newline = match std::fs::read(&file) {
        Ok(bytes) => bytes.last().is_some_and(|b| *b != b'\n'),
        Err(_) => false,
    };
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)?;
    if needs_newline {
        out.write_all(b"\n")?;
    }
    let mut line = serde_json::to_string(entry)?;
    line.push('\n');
    out.write_all(line.as_bytes())?;
    Ok(())
}

/// Every entry of a card's log, oldest first. A missing file is an empty log;
/// lines that do not parse (a torn last line after a crash) are skipped.
pub fn read(paths: &Paths, card_id: &str) -> Result<Vec<LogEntry>> {
    let text = match std::fs::read_to_string(log_file_for(paths, card_id)?) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    Ok(text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;

    #[test]
    fn the_log_is_append_only_jsonl_and_skips_a_torn_line() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::at(dir.path());
        let e = LogEntry {
            id: "1".into(),
            owner: "captain".into(),
            at: 5,
            kind: "note".into(),
            text: "waiting on PR #25".into(),
            links: vec![],
            session_id: None,
        };
        append(&paths, "c1", &e).unwrap();
        append(
            &paths,
            "c1",
            &LogEntry {
                id: "2".into(),
                ..e.clone()
            },
        )
        .unwrap();
        // A half-written line (crash mid-append) must not break the read.
        let file = paths.log_file("c1");
        let raw = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, format!("{raw}{{\"id\":\"3\"")).unwrap();
        let entries = read(&paths, "c1").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "waiting on PR #25");
        // Appending after the torn line must not lose the new entry.
        append(
            &paths,
            "c1",
            &LogEntry {
                id: "4".into(),
                ..e.clone()
            },
        )
        .unwrap();
        assert_eq!(read(&paths, "c1").unwrap().len(), 3);
    }

    #[test]
    fn read_refuses_a_card_id_that_escapes_the_logs_dir() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::at(dir.path());
        assert!(read(&paths, "c1/../escape").is_err());
        assert!(read(&paths, "..").is_err());
    }

    #[test]
    fn log_entries_keep_their_owner_and_session() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::at(dir.path());
        let captain = LogEntry {
            id: "1".into(),
            owner: "captain".into(),
            at: 1,
            kind: "note".into(),
            text: "started the fleet refactor".into(),
            links: vec![],
            session_id: None,
        };
        let agent = LogEntry {
            id: "2".into(),
            owner: "barclay".into(),
            at: 2,
            kind: "result".into(),
            text: "PR opened".into(),
            links: vec!["https://example.test/pr/1".into()],
            session_id: Some("l1".into()),
        };
        append(&paths, "c1", &captain).unwrap();
        append(&paths, "c1", &agent).unwrap();
        let entries = read(&paths, "c1").unwrap();
        assert_eq!(entries[0].owner, "captain");
        assert_eq!(entries[1].owner, "barclay");
        // The session id is the pointer back into the harness's own transcript.
        assert_eq!(entries[1].session_id.as_deref(), Some("l1"));
        assert!(append(&paths, "c1/../escape", &captain).is_err());
    }
}
