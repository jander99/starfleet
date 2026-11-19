//! Pull-request artifacts: parse a PR URL, ask `gh` for its state, and
//! poll those URLs on a paced, capped schedule.

use crate::env::{sh_quote, HostEnv};
use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;

/// The last poll's answer for a PR, as the spec's §8 vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrStatus {
    /// Queued, running, or no checks yet.
    CiRunning,
    CiPassed,
    CiFailed,
    /// Checks failed but at least one is still running.
    CiPending,
    Merged,
    Closed,
    Draft,
    /// Open with checks that finished in a mix of pass and non-fail.
    Open,
}

impl PrStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            PrStatus::CiRunning => "ci_running",
            PrStatus::CiPassed => "ci_passed",
            PrStatus::CiFailed => "ci_failed",
            PrStatus::CiPending => "ci_pending",
            PrStatus::Merged => "merged",
            PrStatus::Closed => "closed",
            PrStatus::Draft => "draft",
            PrStatus::Open => "open",
        }
    }
}

/// `owner/repo#number` from a GitHub pull URL. Only pull URLs count.
pub fn parse_pr_url(url: &str) -> Option<(String, String, u64)> {
    let rest = url.strip_prefix("https://github.com/")?;
    let mut it = rest.split('/');
    let owner = it.next()?;
    let repo = it.next()?.trim_end_matches(".git");
    let kind = it.next()?;
    let number = it.next()?.parse().ok()?;
    if kind != "pull" || owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string(), number))
}

/// A PR's state from `gh pr view --json`. `state` is the whole answer:
/// `checks` says whether CI is running, passing, failing or pending.
pub fn status_from_gh(v: &Value) -> PrStatus {
    let state = v
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_uppercase();
    if state == "MERGED" {
        return PrStatus::Merged;
    }
    if state == "CLOSED" {
        return PrStatus::Closed;
    }
    if v.get("isDraft").and_then(Value::as_bool).unwrap_or(false) {
        return PrStatus::Draft;
    }
    let checks = v.get("checks");
    let check_state = checks
        .and_then(|c| c.get("state"))
        .and_then(Value::as_str)
        .unwrap_or("");
    match check_state.to_uppercase().as_str() {
        "FAILURE" | "ERROR" => PrStatus::CiFailed,
        "PENDING" => PrStatus::CiRunning,
        "SUCCESS" => PrStatus::CiPassed,
        _ => {
            let passing = checks
                .and_then(|c| c.get("passing"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if passing > 0 {
                PrStatus::Open
            } else {
                PrStatus::CiRunning
            }
        }
    }
}

/// Statuses with a CI verdict still in flight get the fast cadence.
pub fn is_active(s: PrStatus) -> bool {
    matches!(
        s,
        PrStatus::CiRunning | PrStatus::CiPending | PrStatus::Open | PrStatus::Draft
    )
}

/// `gh pr view <n> -R <o>/<r> --json state,isDraft,checks,url`, run through
/// `env`. `gh` is resolved from PATH by the shell.
pub async fn fetch_pr(env: &HostEnv, gh: Option<&str>, url: (&str, &str, u64)) -> Result<Value> {
    let (owner, repo, number) = url;
    let bin = gh.map(sh_quote).unwrap_or_else(|| "gh".to_string());
    let script =
        format!("{bin} pr view {number} -R {owner}/{repo} --json state,isDraft,checks,url");
    let out = env.sh(&script).await?;
    if !out.ok {
        let msg = if out.stderr.is_empty() {
            out.stdout.clone()
        } else {
            out.stderr.clone()
        };
        return Err(anyhow::anyhow!(msg));
    }
    if out.stdout.is_empty() {
        return Err(anyhow::anyhow!(
            "gh printed nothing for {owner}/{repo}#{number}"
        ));
    }
    serde_json::from_str(&out.stdout).map_err(|e| anyhow::anyhow!("gh's JSON did not parse: {e}"))
}

/// One poll of one PR: its status, or the error `gh` reported.
pub async fn poll_pr(env: &HostEnv, gh: Option<&str>, url: &str) -> Result<PrStatus> {
    let parsed =
        parse_pr_url(url).ok_or_else(|| anyhow::anyhow!("not a pull-request URL: {url}"))?;
    let v = fetch_pr(env, gh, (&parsed.0, &parsed.1, parsed.2)).await?;
    Ok(status_from_gh(&v))
}

/// Paces PR polls: every URL is due at most every `PR_INTERVAL`, failures
/// back off exponentially up to `MAX_BACKOFF`, and at most `cap` URLs are
/// handed out per batch. A quiet PR (merged, closed) backs off too.
#[derive(Debug)]
pub struct PrPoller {
    cap: usize,
    next_due: BTreeMap<String, u64>,
    backoff: BTreeMap<String, u64>,
}

/// One minute while CI is active.
pub const PR_INTERVAL_MS: u64 = 60_000;
/// Ten minutes is the ceiling for a failing or quiet PR.
pub const MAX_BACKOFF_MS: u64 = 600_000;

impl PrPoller {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            next_due: BTreeMap::new(),
            backoff: BTreeMap::new(),
        }
    }

    /// Which of `urls` may be polled at `now_ms`, soonest-due first.
    pub fn due(&self, urls: &[String], now_ms: u64) -> Vec<String> {
        let mut seen = Vec::new();
        for u in urls {
            if !seen.contains(u) {
                seen.push(u.clone());
            }
        }
        let mut due: Vec<(u64, String)> = seen
            .into_iter()
            .map(|u| (self.next_due.get(&u).copied().unwrap_or(0), u))
            .filter(|(due_at, _)| *due_at <= now_ms)
            .collect();
        due.sort();
        due.into_iter().take(self.cap).map(|(_, u)| u).collect()
    }

    /// Record a poll's outcome at `now_ms` and schedule the next one.
    pub fn record(&mut self, url: &str, outcome: PollOutcome, now_ms: u64) {
        let wait = match outcome {
            PollOutcome::Ok => {
                self.backoff.remove(url);
                PR_INTERVAL_MS
            }
            PollOutcome::RateLimited | PollOutcome::Failed => {
                let next = match self.backoff.get(url) {
                    Some(b) => (*b * 2).min(MAX_BACKOFF_MS),
                    None => 120_000,
                };
                self.backoff.insert(url.to_string(), next);
                next
            }
            // Quiet: nothing left to watch closely.
            PollOutcome::Merged | PollOutcome::Closed => MAX_BACKOFF_MS,
        };
        self.next_due.insert(url.to_string(), now_ms + wait);
    }

    /// Forget URLs no card references any more.
    pub fn retain(&mut self, urls: &[String]) {
        self.next_due.retain(|u, _| urls.contains(u));
        self.backoff.retain(|u, _| urls.contains(u));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PollOutcome {
    Ok,
    RateLimited,
    Failed,
    Merged,
    Closed,
}

/// Turn a `gh` status into a poll outcome: merged/closed are quiet.
pub fn outcome_for(status: PrStatus) -> PollOutcome {
    match status {
        PrStatus::Merged => PollOutcome::Merged,
        PrStatus::Closed => PollOutcome::Closed,
        _ => PollOutcome::Ok,
    }
}

/// A status is stale once it is older than two poll intervals.
pub fn is_stale(checked_at: Option<u64>, now_ms: u64) -> bool {
    checked_at
        .map(|t| now_ms.saturating_sub(t) > 2 * PR_INTERVAL_MS)
        .unwrap_or(true)
}

/// Friendly interval used by the UI, so a long wait reads as "10m".
pub fn human_wait(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m", secs / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pull_url_parses_and_other_urls_do_not() {
        assert_eq!(
            parse_pr_url("https://github.com/o/r/pull/7"),
            Some(("o".into(), "r".into(), 7))
        );
        assert_eq!(parse_pr_url("https://github.com/o/r/issues/7"), None);
        assert_eq!(parse_pr_url("https://github.com/o/r"), None);
        assert_eq!(parse_pr_url("https://example.test/o/r/pull/7"), None);
    }

    #[test]
    fn state_and_checks_map_to_a_status() {
        let s = |v: &str| serde_json::from_str::<Value>(v).unwrap();
        assert_eq!(
            status_from_gh(&s(r#"{"state":"OPEN","checks":{"state":"SUCCESS"}}"#)),
            PrStatus::CiPassed
        );
        assert_eq!(
            status_from_gh(&s(r#"{"state":"OPEN","checks":{"state":"PENDING"}}"#)),
            PrStatus::CiRunning
        );
        assert_eq!(
            status_from_gh(&s(r#"{"state":"OPEN","checks":{"state":"FAILURE"}}"#)),
            PrStatus::CiFailed
        );
        assert_eq!(
            status_from_gh(&s(r#"{"state":"OPEN","isDraft":true}"#)),
            PrStatus::Draft
        );
        assert_eq!(
            status_from_gh(&s(r#"{"state":"OPEN","checks":{"passing":3}}"#)),
            PrStatus::Open
        );
        assert_eq!(
            status_from_gh(&s(r#"{"state":"OPEN","checks":{"passing":0}}"#)),
            PrStatus::CiRunning
        );
        assert_eq!(
            status_from_gh(&s(r#"{"state":"MERGED","checks":{"state":"FAILURE"}}"#)),
            PrStatus::Merged
        );
        assert_eq!(
            status_from_gh(&s(r#"{"state":"CLOSED"}"#)),
            PrStatus::Closed
        );
        // A missing state is not a verdict.
        assert_eq!(status_from_gh(&s(r#"{}"#)), PrStatus::CiRunning);
    }

    #[test]
    fn the_poller_paces_and_caps() {
        let url = "https://github.com/o/r/pull/1".to_string();
        let other = "https://github.com/o/r/pull/2".to_string();
        let mut p = PrPoller::new(2);
        assert_eq!(
            p.due(&[url.clone(), other.clone(), url.clone()], 0),
            vec![url.clone(), other.clone()]
        );
        p.record(&url, PollOutcome::Ok, 0);
        assert_eq!(
            p.due(&[url.clone(), other.clone()], 30_000),
            vec![other.clone()]
        );
        assert_eq!(
            p.due(&[url.clone(), other.clone()], 60_000),
            vec![other.clone(), url.clone()]
        );

        // The cap bounds one batch.
        let capped = PrPoller::new(1);
        assert_eq!(capped.due(&[url.clone(), other.clone()], 0).len(), 1);
    }

    #[test]
    fn failures_back_off_exponentially_up_to_the_cap() {
        let url = "https://github.com/o/r/pull/1".to_string();
        let mut p = PrPoller::new(1);
        let mut t = 0;
        for expected in [120_000, 240_000, 480_000, 600_000, 600_000] {
            assert_eq!(p.due(std::slice::from_ref(&url), t), vec![url.clone()]);
            p.record(&url, PollOutcome::RateLimited, t);
            let due_at = t + expected;
            assert!(
                p.due(std::slice::from_ref(&url), due_at - 1).is_empty(),
                "not before {expected}"
            );
            assert_eq!(p.due(std::slice::from_ref(&url), due_at), vec![url.clone()]);
            t = due_at;
        }
    }

    #[test]
    fn a_merged_pr_is_refreshed_rarely() {
        let url = "https://github.com/o/r/pull/9".to_string();
        let mut q = PrPoller::new(1);
        assert_eq!(q.due(std::slice::from_ref(&url), 0), vec![url.clone()]);
        q.record(&url, PollOutcome::Merged, 0);
        assert!(q.due(std::slice::from_ref(&url), 60_000).is_empty());
        assert_eq!(q.due(std::slice::from_ref(&url), 600_000).len(), 1);
    }

    #[test]
    fn retain_forgets_urls_no_card_holds() {
        let url = "https://github.com/o/r/pull/1".to_string();
        let gone = "https://github.com/o/r/pull/2".to_string();
        let mut p = PrPoller::new(5);
        p.record(&url, PollOutcome::Ok, 0);
        p.record(&gone, PollOutcome::Ok, 0);
        p.retain(std::slice::from_ref(&url));
        assert_eq!(p.due(std::slice::from_ref(&gone), 900_000), vec![gone]);
    }

    #[test]
    fn a_status_checked_long_ago_is_stale() {
        assert!(is_stale(None, 0));
        assert!(!is_stale(Some(100), 100 + PR_INTERVAL_MS));
        assert!(is_stale(Some(100), 100 + 2 * PR_INTERVAL_MS + 1));
        assert_eq!(human_wait(MAX_BACKOFF_MS), "10m");
    }
}

#[tokio::test]
async fn fetch_pr_reports_gh_failures() {
    use crate::env::HostEnv;
    #[cfg(unix)]
    {
        // A failing gh: stderr is preferred as the message.
        let err = fetch_pr(
            &HostEnv::Native,
            Some("sh -c 'echo boom >&2; exit 1'"),
            ("o", "r", 1),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(err.contains("boom"), "{err}");

        // An empty-output gh is its own message.
        let err = fetch_pr(&HostEnv::Native, Some("true"), ("o", "r", 7))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("o/r#7"), "{err}");

        // Valid JSON parses through. `bin` is one shell word, so a
        // multi-word command needs a script on PATH.
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("fake-gh");
        std::fs::write(&bin, "#!/bin/sh\necho '{\"state\":\"OPEN\"}'\n").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let v = fetch_pr(
            &HostEnv::Native,
            Some(&bin.display().to_string()),
            ("o", "r", 2),
        )
        .await
        .unwrap();
        assert_eq!(v["state"], "OPEN");

        // Non-JSON output is reported, not panicked on.
        std::fs::write(&bin, "#!/bin/sh\necho not-json\n").unwrap();
        let err = fetch_pr(
            &HostEnv::Native,
            Some(&bin.display().to_string()),
            ("o", "r", 3),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(err.contains("did not parse"), "{err}");
    }
}

#[test]
fn parse_pr_url_only_takes_pull_links() {
    assert_eq!(
        parse_pr_url("https://github.com/o/r/pull/12"),
        Some(("o".into(), "r".into(), 12))
    );
    assert_eq!(
        parse_pr_url("https://github.com/o/r.git/pull/3"),
        Some(("o".into(), "r".into(), 3))
    );
    assert_eq!(parse_pr_url("https://github.com/o/r/issues/12"), None);
    assert_eq!(parse_pr_url("https://gitlab.com/o/r/pull/12"), None);
    assert_eq!(parse_pr_url("https://github.com/o/r/pull/x"), None);
    assert_eq!(parse_pr_url("https://github.com//r/pull/1"), None);
    assert_eq!(parse_pr_url("https://github.com/o"), None);
}

#[test]
fn status_from_gh_reads_state_draft_and_checks() {
    use serde_json::json;
    assert_eq!(status_from_gh(&json!({"state":"MERGED"})), PrStatus::Merged);
    assert_eq!(status_from_gh(&json!({"state":"closed"})), PrStatus::Closed);
    assert_eq!(
        status_from_gh(&json!({"state":"OPEN","isDraft":true})),
        PrStatus::Draft
    );
    assert_eq!(
        status_from_gh(&json!({"state":"OPEN","checks":{"state":"FAILURE"}})),
        PrStatus::CiFailed
    );
    assert_eq!(
        status_from_gh(&json!({"state":"OPEN","checks":{"state":"error"}})),
        PrStatus::CiFailed
    );
    assert_eq!(
        status_from_gh(&json!({"state":"OPEN","checks":{"state":"PENDING"}})),
        PrStatus::CiRunning
    );
    assert_eq!(
        status_from_gh(&json!({"state":"OPEN","checks":{"state":"SUCCESS"}})),
        PrStatus::CiPassed
    );
    // No check state, but some passing checks: open.
    assert_eq!(
        status_from_gh(&json!({"state":"OPEN","checks":{"passing":2}})),
        PrStatus::Open
    );
    // Nothing known at all: still running.
    assert_eq!(status_from_gh(&json!({})), PrStatus::CiRunning);
}

#[test]
fn active_statuses_are_the_ones_still_in_flight() {
    for s in [
        PrStatus::CiRunning,
        PrStatus::CiPending,
        PrStatus::Open,
        PrStatus::Draft,
    ] {
        assert!(is_active(s), "{s:?}");
    }
    for s in [
        PrStatus::CiPassed,
        PrStatus::CiFailed,
        PrStatus::Merged,
        PrStatus::Closed,
    ] {
        assert!(!is_active(s), "{s:?}");
    }
}

#[test]
fn every_status_has_a_name() {
    for s in [
        PrStatus::CiRunning,
        PrStatus::CiPassed,
        PrStatus::CiFailed,
        PrStatus::CiPending,
        PrStatus::Merged,
        PrStatus::Closed,
        PrStatus::Draft,
        PrStatus::Open,
    ] {
        assert!(!s.as_str().is_empty());
    }
}
