---
title: Starfleet feature inventory
created: 2026-11-13T00:00:00Z
last_modified: 2026-11-13T00:00:00Z
author: Jeff Anderson
type: documentation
tags: [starfleet, features, collaboration]
---

# Starfleet feature inventory

Proposed backlog grounded in the [collaboration research](../research/04-collaboration.md)
and [swarm research](../reports/Agent%20swarm%20orchestration%20patterns.md).
All features below are candidates, not approved specifications or implementation
commitments. Grouping and sequence are recommendations. Agent topology and
autonomous acceptance remain undecided.

## Existing foundation

Station presets and briefs; card-associated sessions; Herdr dispatch and status;
manual follow-ups; pending initial prompts; saved output tails; session drawers.
A card carries a shared working folder. None constitutes durable delegation or
isolated concurrent writing.

## First: prove the collaboration boundary

| ID | Feature | Scope and completion evidence | Dependencies |
| --- | --- | --- | --- |
| F01 | Cross-harness compatibility checks | Manually test one requester/helper exchange, including the image case. Record harness versions, identity propagation, MCP startup, naming, idle wake-up, busy/blocked delivery and failures. Separate observed behavior from unverified claims. | None; no product code needed |
| F02 | Request/result contract | Define task, parent, attempt and message IDs; owner, objective, exclusions, context, artifacts, acceptance criteria and limits. Results carry outcomes, checks, deviations and blockers. Walk through success, failure, missing reply and overlapping requests without ambiguity. | Existing research |
| F03 | Session identity and crew context | Stable managed-session identity and card crew with station/harness/model. Reconcile reconnects and reused pane IDs. Demonstrate that managed callers cannot impersonate another session or address another card. | F01 |
| F04 | Card-scoped comms bridge | Local `starfleet comms` MCP entry point backed by core; crew discovery, send and correlated reply. Authenticate and authorize connections; expose no tools outside managed sessions. Demonstrate a supported cross-harness exchange and reject cross-card access. | F01–F03 |
| F05 | Durable message delivery | Persist messages and delivery state; queue while recipients work or are blocked. Handle offline targets, expiry, duplicates and uncertain delivery. Restart tests must not silently lose messages or blindly replay effects; no unsupported exactly-once guarantee. | F02–F04 |
| F06 | Collaboration history and controls | Show sender, recipient, request, delivery state, artifacts, failures and held messages on the card. Surface approvals and cancellation controls. Trace and intervene in an exchange without reading raw tails. | F02, F05; execution cancellation uses F11 |

## Next: a bounded helper loop

| ID | Feature | Scope and completion evidence | Dependencies |
| --- | --- | --- | --- |
| F07 | Capability discovery and routing | Station capabilities let callers request a deliverable without knowing CLI/model arguments. An `image` request resolves to an eligible station or explains ambiguity/unavailability. | F03 |
| F08 | Per-card authority and permissions | Separate messaging policy from helper-launch policy: off, ask, or explicit station/capability allowlists. Default helper launches to ask. Compare actual cross-harness capabilities, credentials and resources; unknown equivalence requires approval/refusal. Peer text cannot approve requests or expand authority. Define the direct-Herdr/native-message bypass boundary. | F01–F04 |
| F09 | Durable helper requests | Dispatch approved helpers on the card and return tickets immediately. Persist requester ownership, attempts and correlated results. Separate execution, submission, delivery, verification, acceptance and integration; represent blocked/failed/canceled/expired/exhausted attempts. Missing replies remain missing/unknown, never tail-derived success. | F02, F05, F07, F08; F10–F12 required before autonomous use |
| F10 | Enforced execution limits | Bound concurrency, descendants, message rate, hops, retries and runtime. Assess telemetry before promising hard token/spend caps; disclose unenforceable limits. Demonstrate refusal/stopping of a recursive or looping exchange at configured boundaries. | F02, F08; developed alongside F09 |
| F11 | Cancellation and deadlines | Cancel pending launches and queued work; propagate cancellation to managed descendants. Report uncertain outcomes and retained artifacts. Show that cancellation prevents new managed descendants and identifies anything not stopped; no rollback promise. | F02, F05; integrate with F09–F10 |
| F12 | Liveness and restart recovery | Detect abandoned helpers and expired claims; reconcile durable requests with live sessions. Bound retries and reconcile uncertain side effects first. Dead-helper/restart tests must not wait indefinitely or silently duplicate work. | F03, F05, F10; integrate with F09, F11 |

F09–F12 form one safety boundary, not permission to ship autonomous requests
first and add controls later. Message-only automation also requires F08 policy
and applicable F10 limits. The app currently acts on user clicks only.

F17 may later compare a bounded manager with a durable workflow graph, as the
swarm report proposes. That topology comparison is deferred until the basic
exchange is proven; neither architecture is selected here.

## Before concurrent coding and trusted acceptance

| ID | Feature | Scope and completion evidence | Dependencies |
| --- | --- | --- | --- |
| F13 | Isolated writer workspaces | Separate card/project identity from writer directory and base revision. Provision worktrees/clones; retain unintegrated work and clean up safely. Two writers must not overwrite the same checkout. Other resources still need separate isolation. | F02, F09; required before parallel code-writing helpers |
| F14 | Artifacts and independent verification | Retain artifact/revision/hash references and exact checks/results. Let a verifier reject against independently owned criteria; authors cannot silently weaken them. Invalidate verdicts after relevant changes. Demonstrate that stale/missing evidence cannot qualify output as verified. | F02, F09; F13 for parallel writers |
| F15 | Acceptance and controlled integration | Separate verification, authorized acceptance and integration. Record who accepted which revision; check the combined result after integration. Completion/reviewer agreement cannot silently accept or integrate output. User-only initial acceptance is proposed, not approved; PR merging remains manual. | F08, F14 |
| F16 | Agent instruction profiles | Requester, bounded specialist and verifier briefs using actual tools and the proven contract. Cover scope, evidence, escalation and untrusted peer content. Validate success, blocker and rejection walkthroughs. Temporary briefs support F01; polished profiles follow the bounded loop. No permanent orchestrator assumed. | F02; production profiles depend on the capabilities they invoke |
| F17 | Reliability and value evaluation | Define representative cases early; compare the bounded loop against a strong single agent before expanding autonomy. Track verified acceptance, regressions, human review time, cost, latency and duplication. Exercise crash, slowness, cancellation and uncertain delivery; keep budget and deadline comparisons distinct. | Cases with F02; evaluation as relevant features become available |

## Later candidates from existing decisions

These are separate from collaboration prerequisites. See the
[deferred decisions](../research/03-decisions.md#deferred).

| ID | Feature | Boundary / unresolved choice |
| --- | --- | --- |
| F18 | Rich card deliverables | Summaries, files and PR links beyond saved tails; choose storage and provenance. |
| F19 | Unassigned-contact UX | Revisit the side tray based on actual use. |
| F20 | Improved sector sprites | Fix documented Enterprise-D and DS9 art limitations. |
| F21 | External sync/plugins | Explore claude-mem cards, Obsidian or cluster integration; no backend selected. |
| F22 | Cross-machine visibility | Explore Herdr federation without assuming shared permissions or account data. |

## Decisions still requiring approval

- Task classes eligible for delegation; whether helpers may request descendants.
- Automatic peer messaging versus approved sends; helper-launch approval scope.
- Context, files, credentials and resources available to each helper.
- Native messaging and direct Herdr launches versus managed-policy guarantees.
- Ownership of acceptance criteria, verification, acceptance and integration.
- Concrete budgets, deadlines, retries and acceptable duplicate-effect risk.
- The first implementation slice after manual compatibility checks.

This inventory does not authorize autonomous actions, automatic acceptance, a
particular framework or a full hierarchy. It is not an implementation plan.
