---
title: The Bridge — Starfleet as a context cockpit for a fleet of projects
created: 2026-11-17T00:00:00Z
type: design
tags: [starfleet, design, bridge, agents, logs, orchestration]
status: draft — design direction; reviewed 2026-11-17; all six decisions made (card identity = A); Windows gh credentials open
---

# The Bridge

**One-liner.** Starfleet is the Captain's view over many parallel coding sessions:
a graphical status board of everything you are working on, where you can start
new work and step back into ongoing work, across projects that may be a git
repo, a meta directory of repos, or a repo inside a meta directory.

Claims below are marked **documented** (a source or the code says so),
**seen** (observed in this repository or on this machine), or
**recommendation** (a design proposal, not established architecture).

## 1. Vocabulary

The metaphor is load-bearing, so pin it down (**recommendation**, from the
Captain's descriptions):

| Term | Meaning |
| --- | --- |
| **Captain** | The person. Owns intent, priorities, and the final decision. |
| **Fleet** | All of the Captain's projects together. |
| **Enterprise** | One project — one ship, one crew. Usually one directory. |
| **Department head** | A crew agent that can do work *or* delegate it (e.g. LaForge, Data, Barclay). |
| **Agent** | A persistent crew identity with a persona, a harness default, and a history. |
| **Session** | One dispatch of an agent: a running process in a Herdr pane, belonging to one card. |
| **Card** | A unit of context — a piece of work with goal, notes, log, artifacts, and the sessions that ran it. |
| **Station** | A post on a ship (a roster entry: post, officer, duty, harness, model, args, prompt). |
| **Bridge** | The UI for one Enterprise: who is on duty, what is happening, what needs the Captain. |
| **Observation Lounge** | The cross-project lens: everything one agent is carrying. |
| **Log** | Append-only record: the Captain's log (yours) and personal logs (per agent). |

Three notes on the words. A **meta directory of microservices is one Enterprise**,
not many — the ship has many systems, but one crew. "Fleet" is reserved for
*all* projects; earlier conversation used it for a set of repos, which is the
same thing only when each repo is its own project. And **Enterprise is also a
site name** in the current roster (`src/components/MapView.tsx` renders
academy, daystrom, enterprise, ds9), so the project reading collides with the
location reading; the roster's site should be renamed before the metaphor is
taken into code. Likewise **Station.agent** is today the harness name
("claude") — the persona identity is a new concept, so that field becomes
`harness`.

## 2. Data model

Grounded in the current core (**seen**: `crates/core/src/store.rs`,
`crates/core/src/stations.rs`, `crates/core/src/herdr/model.rs`).

```
Project   { path, name, kind }            kind: repo | meta | repo_in_meta
Card      { id, title, cwd, notes, done, artifacts[], captain_log[] }
Agent     { id, name, persona, rank, department, default_harness }
Station   { id, location_id, post, name, duty, harness, model, args, prompt }
Session   { id, card_id, agent_id, station_id, cwd, pane_id?, terminal_id?,
            agent_session? {source, agent, kind, value}, resumed_from?,
            prompt, pending_prompt?, last_status, last_seen, output?, output_at?,
            end_reason?, ended_at?, cause_hint? }   end_reason: captain | idle_timeout
                                                    | interrupted (§8.1)
LogEntry  { id, owner, at, kind, text, links[], session_id? }
Artifact  { kind, label, target, last_status?, checked_at? }
```

- `Session` **is** today's `SessionLink` — it already carries `card_id`,
  `station_id`, `prompt`, `pending_prompt`, `terminal_id`, `pane_id`,
  `agent_session`, `last_status`, `last_seen`, `output`, `output_at`.
- `agent_session` is the harness's own session handle. Herdr reports it
  (`{"source":"herdr:claude","agent":"claude","kind":"id","value":"007c7218-…"}`),
  which is what makes resume and automatic history possible for claude and
  opencode (**seen**: live `herdr agent list`).
- `Project.kind` is new. `~/workspaces` has all three shapes: single repos
  (`starfleet`, `homelab`, `tlh`, `jeffanderson.dev`, …), a meta directory with
  several repos (`ai`, 3), and a meta directory with one (`finance`)
  (**seen**). A session can be started in any of them.
- `LogEntry` and `Artifact` are new.

One dataset, three groupings: **project** (directory), **agent** (identity),
**session** (one running thing). A card belongs to one project and holds one
or more sessions; whether a resumed session continues a card or opens a new
one is decided as A (§11). The board groups by project; the Lounge filters by
agent; the bridge shows one project. The UX can
be deferred precisely because the model does not change per view.

## 3. The bridge UI

Current chain (**seen**: `src/components/MapView.tsx`, `src/App.tsx`,
`src/components/SessionDrawer.tsx`):

```
SectorMap (locations) -> LocationView (stations) -> station -> sessions -> SessionDrawer
```

Refit (**recommendation**):

```
Sector map  (the fleet: many Enterprises = projects, grouped by directory shape)
  └─ Bridge view (one Enterprise)
      ├─ Captain's console        — your prompt, your log, what needs you
      ├─ Departments              — the crew on duty, each a persona
      │    └─ Persona sheet       — personal log | cards by project | status
      └─ Card console            — output, follow-up, log, artifacts, open actions
           └─ artifact open      — focus Herdr pane | browser | webview | obsidian://
```

- Clicking a **persona** opens the agent, not a session: personal log, his cards
  across projects, current status. This is the Observation Lounge as a sheet —
  a filter over the same board data, not a new room.
- Clicking a **card** opens the session console: the existing drawer, grown to
  include log entries and artifacts.
- The board stays the default view. The bridge is where you *act*; the board is
  where you *triage*.

## 4. Orchestration: the bridge with a log, not a chatty swarm

The question was whether departments talking to each other — collaboration, as
in a crisis on the bridge — is a better architecture than strict delegation.

**Recommendation: keep the hierarchical, mediated design; treat the swarm as a
view and a mental model, not the control plane.** Evidence (**documented**, from
`reports/Agent swarm orchestration patterns.md` and `research/04-collaboration.md`):

- Conversational peer swarms underperform for tightly coupled coding. A
  centralized orchestrator beat a single agent by 90.2% on an internal
  *research* evaluation (Anthropic, June 2025), while multi-agent
  architectures *degraded* SWE-bench Verified by roughly 2–15% and sequential
  planning tasks by 39–70% in the April 2026 scaling study.
- Cursor's autonomous-planner experiment is the archetype of the failure:
  equal peers coordinating through shared tasks lacked end-to-end ownership;
  the fix was a non-coding root planner with narrow workers in separate
  repository copies.
- What separates working swarms is **ownership that survives**: an accountable
  orchestrator delegating narrow, verifiable tasks, while ordinary software
  owns budgets, lifecycle, permissions and approval gates.
- Cross-harness reality: Claude Code has peer messaging and agent teams;
  opencode has no inbox (**documented**). Any agent-to-agent scheme that only
  works in one harness fragments the crew.

So the shape is:

1. **Explicit ownership.** Picard (the Captain, or a Captain agent) owns the
   task and delegates. A department head owns a sub-task. Workers are narrow
   and return results upward. Manager-as-tool, not handoff free-for-all.
2. **Mediated communication.** Heads post to a durable shared log on the card;
   Starfleet — not the agents — decides who is woken with what. Native
   messaging is an accelerator where available, never the foundation.
3. **Software owns the rules.** Budgets, deadlines, permission class, which
   repo a session may write, and the acceptance gate. Agents propose; the
   runtime enforces.
4. **Isolation before concurrency.** Two writers on one repo need separate
   worktrees.
5. **The UI shows the hierarchy.** Who owns whom, who is working, what was
   said — not a soup of peers.

The bridge metaphor supports this: departments do not negotiate directly
mid-crisis; they report to the bridge, which arbitrates.

## 5. Logs

**Recommendation.** Append-only, per owner, durable across panes.

- **Captain's log** — yours. Free text plus decisions: "shelved the swarm as
  control plane", "waiting on PR #23 green", "resume the fleet refactor at the
  migrations". Today this is `card.notes`; promote it to a timeline.
- **Personal logs** — one per agent identity. Each entry:
  `{at, session_id?, kind: prompt|result|artifact|decision, text, links[]}`.
  When the harness exposes a session id, the entry **points at** the harness
  transcript instead of copying it. We keep the pointer and the summary; the
  harness keeps the full history.

The log is durable; the pane is disposable. A card outlives its session and can
still say "Barclay: opened PR #42, CI failing, last entry 3h ago".

## 6. Artifacts and open actions

Each artifact is `{kind, label, target, last_status?, checked_at?}` and *open*
dispatches on kind:

| kind | hand-off (MVP) | embedded (later) |
| --- | --- | --- |
| `herdr_pane` | `focus(pane_id)` — **exists** (`crates/core/src/app.rs`) | terminal widget attached to the pane |
| `pr` | live status via `gh` + token; click opens the browser | webview of the PR page |
| `url` (Confluence, Claude artifact) | default browser | Tauri webview |
| `obsidian` | `obsidian://open?vault=…&file=…` | not worth it — Obsidian is its own app |

Caveats:

- **Embedding a terminal is a project, not a tweak.** The drawer already notes
  that `pane.read` makes an attached Herdr client jump in its scrollback
  (**seen**: comment in `SessionDrawer.tsx`). A real embed means a client in a
  pty speaking Herdr's protocol, with input, resize and escape handling.
  Hand-off to the Herdr window is the MVP; when we do embed, it is xterm.js
  over Herdr attach (§11) — that decision is about *which approach*, not about
  bringing it into the first slice.
- **Webviews are cheap but need auth.** A URL and a window is easy; cookies and
  SSO are the hard part, which the browser hand-off avoids for free.

## 7. The one human in the loop

Attention is the scarce resource, so the UI is shaped by it:

- **Sort by what needs you**, not by activity: `blocked` (agent waiting on a
  question) → `stale` (nothing for hours: resume or kill — may need you more
  than a fresh decision) → `needs decision` (PR green, merged, artifact
  posted) → `working` (glanceable, informational).
- **Staleness is a first-class state.** Lunch or end of day: panes park, the
  card keeps its resume point and log, so you return to *where you were*.
- **Two resume points per card**: the machine's last output, and your last
  sentence. Output reloads the facts; your sentence reloads the intent.
- **Writing one line on the way out is cheap and it is the only thing that
  survives you.** That is the Captain's log's real purpose.

## 8. Lifecycle

- **Resume is resumption of the same session** — same session id, same history
  — even if the model has to reprocess uncached tokens. Cache expiry affects
  cost and latency, not resumability.
- **Idle timeout is a pane policy, not a resumability gate.** Decided: after
  60 minutes idle, park the pane (close it) and keep the session resumable
  from `agent_session`. Warn before parking if the card is mid-task.
- **Parking never applies to a blocked session.** `blocked` is Herdr's own
  reported agent status on the pane (`PaneInfo.agent_status`, already stored as
  `SessionLink.last_status`: blocked / working / idle) — an agent waiting on a
  permission question or a prompt is idle by timestamp but its pane is the one
  thing that needs an answer. Exempt it and nudge instead. "Mid-task" means
  `last_status == working` at the moment the timer fires, or a pending
  (unaccepted) artifact change.
- **Reopen is core's job, triggered by the Captain** (decided): parking
  closes the pane; resuming means a new pane running the harness's own resume
  command — `claude --resume <id>` for claude, `opencode --session <id>` for
  opencode (flag to verify against the installed version). The card shows an
  acceptance-style warning button ("this session was parked — reopen to
  continue"), and clicking it is the trigger. **If `agent_session` was never
  reported, the button is disabled** with an explanation: there is no id to
  resume, only a saved output tail. The Herdr call is the normal dispatch
  path with the saved session id attached.
### 8.1 Paused vs interrupted

**Decided (Captain, 2026-11-17):** there is **no provider fallback**. A persona
is bound to its configured harness and model (per station, via
`Station.model`/`args`). If the provider fails or the usage window runs out,
the session ends; Starfleet does not continue it elsewhere. Failure is an
allowed outcome. What the card must keep is the resume handle.

Herdr cannot tell us why a session stopped: `AgentStatus` is
`idle | working | blocked | done | unknown`, with no provider-health signal
(**seen**: `crates/core/src/herdr/model.rs`). An outage looks like a pane that
went idle after an error, hung at `working`, or vanished (`offline`). So the
distinction is **who caused the stop**, and Starfleet records its own intent:

| State | Cause | Evidence |
| --- | --- | --- |
| **Paused** | The Captain or Starfleet, on purpose | Starfleet stamped `end_reason` when it closed the pane: `captain` or `idle_timeout`. |
| **Interrupted** | Anything else, unplanned | The pane stopped or vanished with no Starfleet stamp. |

A finished turn is **not** an end: the pane stays live and the session is
`idle` or `done`, resumable in place. `end_reason` is set only when a pane
actually closes.

Rules:

- **Intent present → paused; intent absent → interrupted.** Every park and
  reopen is Starfleet's own action, so it stamps `end_reason` and `ended_at`
  at that moment.
- **Idle parking (above) applies only to live sessions.** A session that dies
  on its own was not parked, so it is never stamped `idle_timeout`.
- **Record `agent_session` the moment Herdr first reports it**, not only on
  park. An abrupt death must still leave an id to run `claude --resume <id>`
  once the window reopens. The log entry for the session start carries it.
- **Interrupted cards sort into the `stale` tier (§7).** The Captain decides:
  resume when capacity returns, or abandon. Neither is automatic.
- **`cause_hint?` is optional and heuristic**: a scan of the saved output tail
  for rate-limit or usage-limit text may label `rate_limit`, `usage_window` or
  `outage`. It is shown as "probably", is never used to drive behavior, and is
  a guess like the cache-expiry heuristic below.
- **False interrupted**: a pane the Captain closes by hand in Herdr, outside
  Starfleet, carries no stamp and reads as interrupted. The card offers a
  one-click "I closed this" that relabels it `captain`.

### 8.2 Card state is derived

Under option A (§11.1) a card holds many sessions, so its state is computed,
never stored. **Only the newest session decides** (by `created_at`, following
the `resumed_from` chain):

- newest session **live** → the card shows its status: `blocked`, `working`,
  `idle` or `done`;
- newest session **not live** → `paused` or `interrupted`, from its
  `end_reason`.

An older interrupted session that was later resumed does not count. The
attention sort (§7) runs on this derived state: `blocked` first, then
`interrupted` (the `stale` tier), then `needs decision`, then `working`.

- **Second heuristic** (needs a decision): cache expiry is not observable —
  neither Herdr nor the harness reports it. If we want it, it can only be
  defined as "idle longer than X *and* other sessions are spawning or working
  autonomously" — a guess from idle time, not a reading of the cache.
- **The PR poller lives in core, not in the session** (decided: poll every
  60s while CI is running, backing off when quiet or rate-limited). A card waiting on CI
  green must survive its pane closing: core polls `gh` on an interval, writes
  `last_status`/`checked_at` onto the artifact, and wakes the Captain's view
  when the state changes.
- **Polls are keyed by PR, not by card.** Two cards watching the same PR
  (e.g. a follow-up card and the original) share one poller entry; the
  scheduler keeps one timer per distinct PR URL, and a global cap on
  concurrent `gh` calls protects the rate limit.

## 9. MVP vs deferred

**MVP (slice one).**

- Attach/detach; dispatch; follow-up; focus pane (**exist** today).
- Project-aware cards: `repo | meta | repo_in_meta` (**new** — there is no
  `Project` type in core yet; cards carry a `cwd`), plus a per-session `cwd`
  so a card over a meta directory can aim a session at a subrepo.
- Session console grown: streamed output and follow-up replies.
- `Session.resumed_from` lands in slice one as an optional field (cheap now,
  required by option A); the resume *flow* itself is slice two.
- Captain's log per card — append-only, local.
- One artifact kind live end-to-end: `pr` with status via `gh`, open in browser;
  `herdr_pane` open via focus.
- Attention-sorted board; staleness state.

**Slice two** (still near-term, but not the first cut): personal logs per
agent, idle parking with resume, `claude --resume` reopen.

**Platform caveat.** PR status needs `GITHUB_TOKEN` (or `gh` auth). Linux,
macOS and WSL are the tested path; on Windows the credential story is
unresolved (§12), so on Windows the MVP's PR artifact shows "needs gh auth"
until that is settled — everything else works.

**How the MVP is proven.** Unit tests for the store and logs; `crates/mock-herdr`
for dispatch, focus, parking and the PR artifact's status transitions; an
`crates/core/tests/e2e.rs` run against the mock; and a manual pass against the
live Herdr with a real `gh`. PR polling is tested against recorded `gh`
responses, never the network in CI.

**Deferred.**

- Embedded terminal frames (Ghostty/xterm over Herdr attach).
- Webviews for PR, Confluence, Claude artifacts.
- Cross-agent mediated messaging (the relay), the helper loop, budgets.
  **Cross-harness communication is an explicit MVP non-goal** (Captain,
  2026-11-17): Claude and opencode stations coexist as independent crews that
  do not message each other, and there is no provider fallback (§8.1).
- Isolated worktrees for concurrent writers on one repo.
- Observation Lounge as its own view (a filter first).
- The swarm as a control plane (**recommended against** for now).
- A decision model that routes the Captain's prompt to a department head.

## 10. Storage

**Paths already exist and should be reused** (**seen**: `crates/core/src/paths.rs`).
`Paths::resolve` already does XDG on Linux/macOS (`$XDG_CONFIG_HOME` →
`~/.config`, `$XDG_DATA_HOME` → `~/.local/share`) and Windows
(`%APPDATA%` → config, `%LOCALAPPDATA%` → data), with `STARFLEET_HOME` as an
override for tests and demos. Existing files: `settings.json` and
`roster.json` in `config_dir`, `board.json` in `data_dir`.

Decided (**Captain, 2026-11-17**):

- **Logs go under `data_dir`**, not config: `data_dir/logs/<card_id>.jsonl`,
  append-only JSONL per card. The board stays small; logs grow and are tailed
  cheaply. Personal logs follow the same layout keyed by agent id.
- **The board file stays the index** — cards, sessions, artifacts — and the
  logs are side files referenced from it.
- **Config is JSONC-ish**: the Captain wants comments in `settings.json`.
  `serde_json` rejects comments, so the reader strips `//` and `/* */` before
  parsing (a small pre-pass), keeping the file human-edit-friendly.

**JSONC is a separate feature from storage.** The Captain wants comments in
`settings.json`; a naive strip of `//` and `/* */` breaks on `//` inside
strings (URLs). Use a real JSONC parser — `jsonc-parser` (dprint, MIT, 0.34.0,
`serde` feature) — rather than a hand-written pre-pass.

- **Multi-machine**: the design assumes one machine. Herdr 0.9.5 can attach
  remotely (`--remote <ssh-target>`), but running Starfleet against a remote
  Herdr is a separate project.

## 11. Decisions

1. **Card identity on resume — decided: option A** (same card, new session
   row chained by `resumed_from`; revisit if it does not fit). Options analyzed
   below.

**Decided by the Captain, 2026-11-17:**

2. **Idle timeout**: 60 minutes idle → park the pane, keep the session
   resumable from `agent_session`; warn first if the card is mid-task.
3. **PR poll cadence**: every 60s while CI is running, backing off when the
   PR is quiet or `gh` rate-limits.
4. **Project identity**: a card started in a meta directory owns that meta
   directory as one project; sessions may target subrepos but the card is one
   context.
5. **Terminal embedding**: xterm.js over Herdr attach (a real attach client
   speaking Herdr's protocol in the Tauri webview), not the Ghostty SDK.
6. **Acceptance ownership**: the Captain accepts for the MVP; a verifier
   agent is a later addition.

### 11.1 Card identity on resume

A resume reopens the same harness session (`claude --resume <id>`). The question
is what happens to the *card*, given `SessionLink` already belongs to a card
(`card_id`) and a card can hold several sessions.

| Option | Board shows | Captain's log | History | Cost |
| --- | --- | --- | --- | --- |
| **A. Same card, new session row** (chained by `agent_session`) | one card per ongoing context | one log, entries tagged by session | linear within a card | long cards need round grouping |
| **B. New card per dispatch**, linked to the previous | a card per round | splits across cards | explicit chain | board multiplies; log fragments |
| **C. Same card, same session row mutated** | one card | one log | **lost** — the old session id is overwritten | cheapest, worst |
| **D. Resume forks a child card**, parent archived | parent + child | two logs, child links back | explicit fork | duplicate navigation |

**Chosen: A** (**Captain, 2026-11-17** — "if I don't like it we can change it
later"). The card is the unit of *context*, the session is the
unit of *dispatch* — so resume appends a session with `resumed_from` set to the
old one, and the card keeps your log intact. Rounds are a grouping in the UI
(collapse sessions by time), not separate cards. B and D solve "honest units of
work" by paying with your log, which is the one thing that survives you. C is
the trap: mutating the row silently discards the chain to the old transcript.

What A needs in the model: `Session.resumed_from?`, log entries carry
`session_id`, and the card console collapses old rounds by default.

## 12. Credentials

- **`GITHUB_TOKEN` comes from the environment** (decided). A preflight check
  reports whether it is set, on Linux, macOS and WSL.
- **Windows is open**: `gh` keeps its own `hosts.yml` and Windows stores
  credentials through Git Credential Manager. Until that is figured out, PR
  status is unavailable there; Linux testing resumes first.
- The token is never written into the board file or the logs.

## 13. What this document does not authorize

No implementation. It records a design direction, the evidence behind the
bridge-over-swarm recommendation, and the decisions made so far. Still open:
the Windows `gh` credential story. Card identity is decided (A) and
revisit-able if it does not fit in practice.
