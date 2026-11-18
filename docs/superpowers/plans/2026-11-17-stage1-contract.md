# Stage 1: Core Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the data-model contract — session lifecycle fields, project kind, card artifacts, derived card state — to `crates/core`, tested, with no UI behavior change.

**Architecture:** All changes are additive and serde-defaulted, so a `board.json` written before this lands still loads. Lifecycle intent is persisted on `SessionLink` (`end_reason`, `ended_at`); card state is *derived* in `view.rs` from the newest session; project kind is computed from the filesystem path and never persisted.

**Tech Stack:** Rust (crates/core), serde/serde_json, tempfile, mock-herdr, hand-mirrored TypeScript contract in `src/api.ts`.

**Spec:** `docs/superpowers/specs/2026-11-17-starfleet-bridge-design.md` — on branch `docs/bridge-design` (PR #25) until merged. Until then read it with `git show origin/docs/bridge-design:docs/superpowers/specs/2026-11-17-starfleet-bridge-design.md`.

## Global Constraints

- Every new field on a persisted struct carries `#[serde(default)]` (or the struct already has it): an old `board.json` must load unchanged.
- JSON keys are snake_case. `src/api.ts` is mirrored by hand — Rust field names and TS interface fields must match exactly.
- No new crate dependencies in stage 1.
- Unknown enum values deserialize to a fallback variant; a bad value never fails the board load.
- `project_kind` takes user paths and must be total: no panic on a missing path, a file, or an unreadable directory.
- No UI behavior ships. Both `cargo test --workspace` and `npm test` must pass at the end of every task.

## Review Focus

Each line gets its test in the task that owns the code.

1. **An existing `board.json` written before this change still loads and saves.**
2. **Equal `created_at` between two sessions** — the newest must be chosen deterministically, or the card state flickers between views.
3. **A live blocked session older than the newest** — a permission prompt must not be hidden by §8.2's newest-wins rule; blocked aggregates across live sessions.
4. **`resumed_from` with a dangling target or a cycle** — the chain walk must terminate.
5. **`project_kind` on a path that does not exist, is a file, is a repo inside a meta dir, or is a meta dir that is itself a repo.**

---

## File Structure

- `crates/core/src/store.rs` — Modify: `SessionLink` lifecycle fields, `EndReason`, `Artifact`/`ArtifactKind` on `Card`, `resume_chain`, `Store::{end_session, park_candidates, add_artifact, update_artifact}`. Unit tests live in its existing `mod tests`.
- `crates/core/src/project.rs` — Create: `ProjectKind`, `classify`, `project_kind`. One responsibility: what a directory is.
- `crates/core/src/lib.rs` — Modify: `pub mod project;`.
- `crates/core/src/view.rs` — Modify: `SessionState`, `CardState`, `card_state`, `attention_rank`, new fields on `SessionView`/`CardView`. Card state is a view concern, not a store concern.
- `crates/core/src/stations.rs` — Modify: `Station.agent` → `Station.harness` (with `alias = "agent"`).
- `crates/core/src/app.rs` — Modify: call sites of `station.agent`.
- `crates/core/tests/e2e.rs` — Modify: one interrupted-session test against mock-herdr.
- `src/api.ts`, `src/mock.ts`, `src/__tests__/mock.test.ts` — Modify: mirror the additive contract.
- `docs/superpowers/specs/2026-11-17-starfleet-bridge-design.md` — Modify: §8.2 note on aggregated blocked (Task 4).

**Branch:** `feat/stage1-contract` from `origin/main` (`.worktrees/stage1-contract` per AGENTS.md).

---

### Task 1: Session lifecycle fields

**Files:**
- Modify: `crates/core/src/store.rs:39-66` (`SessionLink`)
- Test: `crates/core/src/store.rs` (`mod tests`, bottom of file)

**Interfaces:**
- Consumes: nothing.
- Produces:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    Captain,
    IdleTimeout,
    Interrupted,
    #[default]
    #[serde(other)]
    Unknown,
}

// added to SessionLink:
pub resumed_from: Option<String>,
pub cwd: Option<String>,
pub end_reason: Option<EndReason>,
pub ended_at: Option<u64>,
```

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_board_written_before_the_lifecycle_fields_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("board.json");
    std::fs::write(
        &file,
        r#"{"cards":[{"id":"c1","title":"t","cwd":"/w","notes":"","done":false,"created_at":1,"updated_at":1}],
            "links":[{"id":"l1","card_id":"c1","created_at":2}]}"#,
    )
    .unwrap();
    let store = Store::load(file).unwrap();
    let link = &store.board.links[0];
    assert_eq!(link.resumed_from, None);
    assert_eq!(link.cwd, None);
    assert_eq!(link.end_reason, None);
    assert_eq!(link.ended_at, None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p starfleet-core a_board_written_before -- --nocapture`
Expected: FAIL — the fields do not exist yet.

- [ ] **Step 3: Add `EndReason` and the four fields to `SessionLink`**

`SessionLink` already carries `#[serde(default)]`, so no per-field attribute is needed. Put `EndReason` next to it in `store.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p starfleet-core a_board_written_before`
Expected: PASS.

- [ ] **Step 5: Write the fallback test**

```rust
#[test]
fn an_end_reason_round_trips_and_unknown_values_fall_back() {
    let cases = [
        ("\"captain\"", EndReason::Captain),
        ("\"idle_timeout\"", EndReason::IdleTimeout),
        ("\"interrupted\"", EndReason::Interrupted),
        ("\"provider_outage\"", EndReason::Unknown),
    ];
    for (json, expected) in cases {
        assert_eq!(serde_json::from_str::<EndReason>(json).unwrap(), expected);
    }
    assert_eq!(serde_json::to_string(&EndReason::IdleTimeout).unwrap(), "\"idle_timeout\"");
}
```

- [ ] **Step 6: Run it, then commit**

Run: `cargo test -p starfleet-core an_end_reason_round_trips` → PASS.

```bash
git add crates/core/src/store.rs
git commit -m "feat(core): session lifecycle fields — resumed_from, cwd, end_reason, ended_at"
```

---

### Task 2: Project kind

**Files:**
- Create: `crates/core/src/project.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod project;`)
- Test: `crates/core/src/project.rs` (`mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind { Repo, Meta, RepoInMeta }

pub fn classify(is_git_root: bool, child_repos: usize, parent_is_meta: bool) -> ProjectKind;
pub fn project_kind(path: &Path) -> ProjectKind;
```

Rules, exactly:
- `is_git_root && child_repos > 0` → `Meta` (a meta dir that is itself a repo)
- `is_git_root && parent_is_meta` → `RepoInMeta`
- `is_git_root` → `Repo`
- otherwise → `Meta` (a plain directory, empty or not — a path we cannot read is treated as a plain project directory)

Definitions: a *git root* is a path where `.git` exists as a directory **or a file** (worktrees and submodules use a file). `child_repos` counts immediate child directories containing `.git`. `parent_is_meta` means the parent has no `.git` and at least two child directories containing `.git`.

- [ ] **Step 1: Write the failing pure test**

```rust
#[test]
fn classify_follows_the_rules() {
    assert_eq!(classify(true, 0, false), ProjectKind::Repo);
    assert_eq!(classify(true, 0, true), ProjectKind::RepoInMeta);
    assert_eq!(classify(true, 3, false), ProjectKind::Meta);
    assert_eq!(classify(false, 3, false), ProjectKind::Meta);
    assert_eq!(classify(false, 0, false), ProjectKind::Meta);
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p starfleet-core classify_follows_the_rules`
Expected: FAIL — `project` module does not exist.

- [ ] **Step 3: Implement `classify` and `project_kind`**

`classify` is the pure rule table above. `project_kind` reads the filesystem and calls it: `is_git_root = path.join(".git").exists()`, `child_repos = read_dir(path)` counting immediate children that are directories with `.git`, `parent_is_meta` from the parent the same way (no `.git`, ≥2 child repos). Every `read_dir`/metadata error counts as "no repos" — never `unwrap`.

- [ ] **Step 4: Write the IO test (Review Focus 5)**

```rust
#[test]
fn a_fleet_of_repos_is_a_meta_project() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["svc-a", "svc-b"] {
        std::fs::create_dir_all(dir.path().join(name).join(".git")).unwrap();
    }
    assert_eq!(project_kind(dir.path()), ProjectKind::Meta);
    assert_eq!(project_kind(&dir.path().join("svc-a")), ProjectKind::RepoInMeta);
    // Totality: missing path, and a path that is a file.
    assert_eq!(project_kind(&dir.path().join("missing")), ProjectKind::Meta);
    let file = dir.path().join("a.txt");
    std::fs::write(&file, "x").unwrap();
    assert_eq!(project_kind(&file), ProjectKind::Meta);
}
```

- [ ] **Step 5: Run and commit**

Run: `cargo test -p starfleet-core project::` → PASS.

```bash
git add crates/core/src/project.rs crates/core/src/lib.rs
git commit -m "feat(core): classify a project path as repo, meta or repo-in-meta"
```

---

### Task 3: Card artifacts

**Files:**
- Modify: `crates/core/src/store.rs` (`Card`, `impl Store`)
- Test: `crates/core/src/store.rs` (`mod tests`)

**Interfaces:**
- Consumes: Task 1's file state (same file — do Task 1 first).
- Produces:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind { Pr, Url, Note, Pane }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub kind: ArtifactKind,
    pub label: String,
    pub target: String,
    pub last_status: Option<String>,
    pub checked_at: Option<u64>,
}

// Card gains: pub artifacts: Vec<Artifact>   (serde default)
impl Store {
    pub fn add_artifact(&mut self, card_id: &str, artifact: Artifact) -> Result<()>;
    pub fn update_artifact(&mut self, card_id: &str, artifact_id: &str,
                           last_status: Option<&str>, checked_at: Option<u64>) -> Result<()>;
}
```

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn an_artifact_survives_a_reload() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("board.json");
    let mut store = Store::load(&file).unwrap();
    let card = store.create_card("pr work", "/w", "").unwrap();
    store
        .add_artifact(
            &card.id,
            Artifact {
                id: "a1".into(),
                kind: ArtifactKind::Pr,
                label: "PR #25".into(),
                target: "https://github.com/jander99/starfleet/pull/25".into(),
                last_status: None,
                checked_at: None,
            },
        )
        .unwrap();
    store.update_artifact(&card.id, "a1", Some("ci_running"), Some(42)).unwrap();
    let reloaded = Store::load(&file).unwrap();
    let a = &reloaded.card(&card.id).unwrap().artifacts[0];
    assert_eq!(a.last_status.as_deref(), Some("ci_running"));
    assert_eq!(a.checked_at, Some(42));
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p starfleet-core an_artifact_survives_a_reload`
Expected: FAIL — `Artifact` does not exist.

- [ ] **Step 3: Implement `Artifact`, `ArtifactKind`, `Card.artifacts` and the two store methods**

`add_artifact` calls `self.card(card_id)?` first (unknown card → `Err`), pushes, then `self.save()`. `update_artifact` finds the artifact by id and returns `Err(anyhow!("that artifact no longer exists"))` if missing — never panic. Both save.

- [ ] **Step 4: Add the error test**

```rust
#[test]
fn updating_an_unknown_artifact_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::load(dir.path().join("board.json")).unwrap();
    let card = store.create_card("c", "/w", "").unwrap();
    assert!(store.update_artifact(&card.id, "nope", Some("x"), None).is_err());
}
```

- [ ] **Step 5: Run and commit**

Run: `cargo test -p starfleet-core artifact` → PASS.

```bash
git add crates/core/src/store.rs
git commit -m "feat(core): artifacts on a card (pr, url, note, pane) with store updates"
```

---

### Task 4: Derived card state

**Files:**
- Modify: `crates/core/src/view.rs`
- Modify: `docs/superpowers/specs/2026-11-17-starfleet-bridge-design.md` (§8.2)
- Test: `crates/core/src/view.rs` (`mod tests`, add one if absent)

**Interfaces:**
- Consumes: `EndReason` (Task 1), `SessionStatus` (exists, `view.rs:11`).
- Produces:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionState {
    pub id: String,
    pub created_at: u64,
    pub live: bool,
    pub status: SessionStatus,
    pub end_reason: Option<EndReason>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardState { Empty, Blocked, Working, Idle, Done, Paused, Interrupted, Offline }

pub fn card_state(sessions: &[SessionState]) -> CardState;
pub fn attention_rank(state: CardState) -> u8;

impl SessionView { pub fn state(&self) -> SessionState }
// CardView gains: pub state: CardState, pub kind: ProjectKind
```

Rules, exactly:
- **Newest** = max by `(created_at, id)` — the id tie-break makes it order-independent (Review Focus 2). The `resumed_from` chain is *not* walked here, so a dangling or cyclic chain cannot loop (Review Focus 4; the chain walk lives in Task 5).
- **Blocked aggregates**: if the newest is live and blocked, *or any* live session is blocked → `Blocked`. Otherwise the newest decides (Review Focus 3).
- Newest live → map its status: `Blocked → Blocked`, `Working → Working`, `Done → Done`, `Idle | Unknown → Idle`, `Offline → Offline` (never happens for a live session).
- Newest not live → `end_reason` is `Some(Captain | IdleTimeout)` → `Paused`; `None | Some(Interrupted | Unknown)` → `Interrupted`.
- No sessions → `Empty`.
- `attention_rank`: Blocked 0, Interrupted 1, Done 2, Paused 3, Working 4, Idle 5, Offline 6, Empty 7 — the §7 order (blocked, stale tier, needs decision, working), with `Done` before `Paused` because a finished-unlooked session asks for a decision.

- [ ] **Step 1: Write the failing tests**

```rust
fn ss(id: &str, at: u64, live: bool, status: SessionStatus, reason: Option<EndReason>) -> SessionState {
    SessionState { id: id.into(), created_at: at, live, status, end_reason: reason }
}

#[test]
fn the_newest_session_decides_the_card_state() {
    let older = ss("l2", 20, false, SessionStatus::Offline, Some(EndReason::Interrupted));
    let newer_by_chain = ss("l1", 10, true, SessionStatus::Working, None);
    assert_eq!(card_state(&[older, newer_by_chain]), CardState::Working);
}

#[test]
fn equal_timestamps_are_broken_by_id() {
    let a = ss("a", 10, true, SessionStatus::Idle, None);
    let b = ss("b", 10, true, SessionStatus::Working, None);
    assert_eq!(card_state(&[a.clone(), b.clone()]), CardState::Working);
    assert_eq!(card_state(&[b, a]), CardState::Working); // order-independent
}

#[test]
fn a_blocked_session_hidden_behind_a_newer_one_still_surfaces() {
    let waiting = ss("l1", 10, true, SessionStatus::Blocked, None);
    let newer = ss("l2", 20, true, SessionStatus::Working, None);
    assert_eq!(card_state(&[waiting, newer]), CardState::Blocked);
}

#[test]
fn a_closed_session_is_paused_or_interrupted_by_its_stamp() {
    let paused = ss("l", 1, false, SessionStatus::Offline, Some(EndReason::Captain));
    let died = ss("l", 1, false, SessionStatus::Offline, None);
    assert_eq!(card_state(&[paused]), CardState::Paused);
    assert_eq!(card_state(&[died]), CardState::Interrupted);
    assert_eq!(card_state(&[]), CardState::Empty);
}

#[test]
fn the_attention_order_puts_blocked_first_then_interrupted() {
    assert!(attention_rank(CardState::Blocked) < attention_rank(CardState::Interrupted));
    assert!(attention_rank(CardState::Interrupted) < attention_rank(CardState::Done));
    assert!(attention_rank(CardState::Done) < attention_rank(CardState::Working));
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p starfleet-core card_state`
Expected: FAIL — `CardState` does not exist.

- [ ] **Step 3: Implement `SessionState`, `CardState`, `card_state`, `attention_rank`**

One pass over `sessions`: find the newest by `(created_at, id)`, note whether any live session is blocked, then match per the rules. No recursion.

- [ ] **Step 4: Wire the views**

`SessionView.state()` builds a `SessionState` from its own fields (`link_id` as id, `created_at`, `live`, `status`, and a new `end_reason: Option<EndReason>` field on `SessionView`). `CardView` gains `state: CardState` (from `card_state` over its sessions) and `kind: ProjectKind` (from `project::project_kind(Path::new(&card.cwd))`). Add `end_reason` to `SessionView` in `session_view()` (`crates/core/src/view.rs:101`) for both the live and offline arms.

- [ ] **Step 5: Run the suite and fix the contract**

Run: `cargo test --workspace`
Expected: PASS. If `e2e.rs` asserts on the serialized view, extend the expectation there rather than weakening the test.

- [ ] **Step 6: Update the spec**

In §8.2 of the design doc, after the bullet list, add: *"Blocked aggregates across live sessions: a permission prompt on an older session is never hidden by a newer working one. Everything else is decided by the newest session."*

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/view.rs docs/superpowers/specs/2026-11-17-starfleet-bridge-design.md
git commit -m "feat(core): derive card state and project kind for the board view"
```

---

### Task 5: Resume chain, end stamping, parking candidates

**Files:**
- Modify: `crates/core/src/store.rs`
- Test: `crates/core/src/store.rs` (`mod tests`)

**Interfaces:**
- Consumes: `SessionLink` (Task 1), `EndReason` (Task 1), `AgentStatus` (`herdr/model.rs:11`).
- Produces:

```rust
pub fn resume_chain(links: &[SessionLink], start: &str) -> Vec<String>;

impl Store {
    pub fn end_session(&mut self, link_id: &str, reason: EndReason) -> Result<()>;
    /// Link ids eligible for idle parking: live, not blocked, last seen at least
    /// `idle_ms` before `now`.
    pub fn park_candidates(&self, now: u64, idle_ms: u64) -> Vec<String>;
}
```

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_resume_chain_stops_at_a_dangling_id() {
    let links = vec![
        SessionLink { id: "a".into(), resumed_from: None, ..Default::default() },
        SessionLink { id: "b".into(), resumed_from: Some("a".into()), ..Default::default() },
        SessionLink { id: "c".into(), resumed_from: Some("missing".into()), ..Default::default() },
    ];
    assert_eq!(resume_chain(&links, "b"), vec!["b".to_string(), "a".to_string()]);
    assert_eq!(resume_chain(&links, "c"), vec!["c".to_string()]);
    assert_eq!(resume_chain(&links, "nope"), Vec::<String>::new());
}

#[test]
fn a_cyclic_resume_chain_does_not_loop() {
    let links = vec![
        SessionLink { id: "a".into(), resumed_from: Some("b".into()), ..Default::default() },
        SessionLink { id: "b".into(), resumed_from: Some("a".into()), ..Default::default() },
    ];
    let chain = resume_chain(&links, "a");
    assert_eq!(chain.len(), 2);
}

#[test]
fn parking_skips_blocked_and_recent_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::load(dir.path().join("board.json")).unwrap();
    for (id, seen, status) in [
        ("old", 0u64, AgentStatus::Idle),
        ("fresh", 9_000, AgentStatus::Idle),
        ("waiting", 0, AgentStatus::Blocked),
        ("closed", 0, AgentStatus::Done),
    ] {
        let mut link = SessionLink {
            id: id.into(),
            last_seen: Some(seen),
            last_status: status,
            ..Default::default()
        };
        if id == "closed" {
            link.end_reason = Some(EndReason::Captain);
            link.ended_at = Some(1);
        }
        store.board.links.push(link);
    }
    assert_eq!(store.park_candidates(10_000, 3_600_000), vec!["old".to_string()]);
}

#[test]
fn ending_a_session_stamps_reason_and_time_and_persists() {
    // create card, dispatch-free link via store_with(link), then:
    // store.end_session("l1", EndReason::IdleTimeout).unwrap();
    // Store::load(file) -> link.end_reason == Some(EndReason::IdleTimeout)
    //                    -> link.ended_at.is_some()
    // store.end_session("missing", EndReason::Captain).is_err()
}
```

`park_candidates` is a pure store query over link fields — it does not read a Herdr snapshot. Liveness is checked by the caller (`app.rs`) at park time; the doc comment on the method must say so. A link already ended (`end_reason.is_some()`) is never a candidate.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p starfleet-core resume_chain parking_skips ending_a_session`
Expected: FAIL — functions do not exist.

- [ ] **Step 3: Implement**

`resume_chain`: walk `resumed_from` from `start`, collecting ids into a `HashSet` guard; stop when the id is unknown, already visited, or the chain exceeds 1000 entries. `end_session`: find the link, set `end_reason` and `ended_at: Some(now_ms())`, save; unknown id → `Err(anyhow!("that session no longer exists"))`. `park_candidates`: links where `end_reason.is_none()`, `last_status != AgentStatus::Blocked`, `last_seen` is `Some(t)` and `t + idle_ms <= now` — deterministic order (the order links appear in the board), no snapshot read.

- [ ] **Step 4: Run the suite and commit**

Run: `cargo test --workspace` → PASS.

```bash
git add crates/core/src/store.rs
git commit -m "feat(core): resume chain walk, end-session stamping, parking candidates"
```

---

### Task 6: Log contract

**Files:**
- Create: `crates/core/src/log.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod log;`), `crates/core/src/paths.rs` (`logs_dir()`)
- Test: `crates/core/src/log.rs` (`mod tests`)

**Interfaces:**
- Consumes: `crates/core/src/paths.rs`'s `Paths` (`data_dir`).
- Produces:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub owner: String,          // "captain" or an agent id
    pub at: u64,
    pub kind: String,           // "prompt" | "result" | "artifact" | "decision" | "note"
    pub text: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Append-only JSONL at `<data_dir>/logs/<card_id>.jsonl`.
pub fn append(paths: &Paths, card_id: &str, entry: &LogEntry) -> Result<()>;
pub fn read(paths: &Paths, card_id: &str) -> Result<Vec<LogEntry>>;
```

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn the_log_is_append_only_jsonl_and_skips_a_torn_line() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::at(dir.path());  // use the constructor paths.rs already exposes
    let e = LogEntry {
        id: "1".into(), owner: "captain".into(), at: 5, kind: "note".into(),
        text: "waiting on PR #25".into(), links: vec![], session_id: None,
    };
    append(&paths, "c1", &e).unwrap();
    append(&paths, "c1", &LogEntry { id: "2".into(), ..e }).unwrap();
    // A half-written line (crash mid-append) must not break the read.
    let file = paths.log_file("c1");
    let raw = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{raw}{{\"id\":\"3\"")).unwrap();
    let entries = read(&paths, "c1").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].text, "waiting on PR #25");
}
```

(Check `paths.rs` for the real constructor and add `log_file(card_id)` + `logs_dir()` next to `board_file` at `crates/core/src/paths.rs:61`; adjust the test if it exposes a different one.)

- [ ] **Step 2: Run it**

Run: `cargo test -p starfleet-core the_log_is_append_only` — Expected: FAIL.

- [ ] **Step 3: Implement**

`append` creates `logs_dir()` if missing, opens the file in append mode, writes one `serde_json::to_string(entry)` line. `read` returns `[]` for a missing file, and skips lines that fail to parse instead of erroring (a torn last line is expected after a crash).

- [ ] **Step 4: Add the ownership test**

```rust
#[test]
fn log_entries_keep_their_owner_and_session() {
    // Append one captain entry and one agent entry with session_id Some("l1"),
    // read back, assert owners are "captain" and the agent id, and that
    // the session_id survived — this is the pointer back into the harness transcript.
}
```

- [ ] **Step 5: Run and commit**

Run: `cargo test -p starfleet-core log::` → PASS.

```bash
git add crates/core/src/log.rs crates/core/src/paths.rs crates/core/src/lib.rs
git commit -m "feat(core): append-only JSONL log per card — captain and personal logs"
```

---

### Task 7: Interrupted session end-to-end

**Files:**
- Modify: `crates/core/tests/e2e.rs`
- Test: `crates/core/tests/e2e.rs`

**Interfaces:**
- Consumes: `Store::reconcile`, `card_state`, `EndReason`, mock-herdr.
- Produces: nothing new — proves the contract against a real Herdr protocol fixture.

- [ ] **Step 1: Write the failing test**

Follow the harness in `crates/core/tests/e2e.rs` (`until` at :56, dispatch at :87) and the shape of `herdr_going_away_is_shown_as_disconnected` (:341). The new test: dispatch a session, wait until it is live, remove that pane from mock-herdr, reconcile once, then assert the link is not live, `end_reason.is_none()` (nobody stamped it), and `card_state` for that card is `Interrupted` — not `Paused`.

- [ ] **Step 2: Run it**

Run: `cargo test -p starfleet-core interrupted` — Expected: FAIL until Step 3.

- [ ] **Step 3: Make it pass**

The data path should already work after Task 4: no stamp → interrupted. If it fails, the bug is in `card_state`'s not-live arm — fix there, not in the test.

- [ ] **Step 4: Run and commit**

Run: `cargo test --workspace` → PASS.

```bash
git add crates/core/tests/e2e.rs
git commit -m "test(core): a session whose pane vanishes reads as interrupted"
```

---

### Task 8: Station field rename and the frontend mirror

**Files:**
- Modify: `crates/core/src/stations.rs:15` (`Station.agent` → `harness`), `crates/core/src/app.rs` (call sites), `src/api.ts`, `src/mock.ts`, `src/__tests__/mock.test.ts`
- Test: `crates/core/src/stations.rs` (`mod tests`), `src/__tests__/mock.test.ts`

**Interfaces:**
- Consumes: the JSON contract from Tasks 1, 3, 4.
- Produces: `Station.harness` (string, harness kind), TS mirrors:

```ts
export type SessionStatus = "blocked" | "done" | "working" | "idle" | "unknown" | "offline";
export type EndReason = "captain" | "idle_timeout" | "interrupted" | "unknown";
export type CardState = "empty" | "blocked" | "working" | "idle" | "done" | "paused" | "interrupted" | "offline";
export type ProjectKind = "repo" | "meta" | "repo_in_meta";
// Session gains: resumed_from?: string | null; cwd?: string | null;
//                end_reason?: EndReason | null;
// Card gains:   artifacts: Artifact[]; state: CardState; kind: ProjectKind;
```

- [ ] **Step 1: Write the failing Rust test**

```rust
#[test]
fn the_old_agent_key_still_parses() {
    let json = r#"{"id":"s","name":"Data","agent":"claude"}"#;
    let st: Station = serde_json::from_str(json).unwrap();
    assert_eq!(st.harness, "claude");
    let out = serde_json::to_string(&st).unwrap();
    assert!(out.contains("\"harness\""));
    assert!(!out.contains("\"agent\""));
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p starfleet-core the_old_agent_key_still_parses` — Expected: FAIL.

- [ ] **Step 3: Rename with a serde alias**

`#[serde(alias = "agent")] pub harness: String,` and update every `station.agent` call site (`grep -rn "station\.agent\|s\.agent" crates/core/src`). `alias` affects deserialization only; new writes use `harness`.

- [ ] **Step 4: Write the failing frontend test**

```ts
it("cards carry a derived state and a project kind", async () => {
  const v = await createMockBackend().view();
  for (const c of v.cards) {
    expect(["empty", "blocked", "working", "idle", "done", "paused", "interrupted", "offline"]).toContain(c.state);
    expect(["repo", "meta", "repo_in_meta"]).toContain(c.kind);
    expect(Array.isArray(c.artifacts)).toBe(true);
  }
});
```

- [ ] **Step 5: Mirror the contract in `src/api.ts` and `src/mock.ts`**

Add the optional fields to `Session`, `state`/`kind`/`artifacts` to `Card`, and populate them in the mock's card/session builders so the test passes.

- [ ] **Step 6: Run everything and commit**

Run: `cargo test --workspace && npm test` → PASS.

```bash
git add crates/core/src/stations.rs crates/core/src/app.rs src/api.ts src/mock.ts src/__tests__/mock.test.ts
git commit -m "feat: station harness rename (aliased) and mirrored contract in the frontend"
```
