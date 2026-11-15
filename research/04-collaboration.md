# Getting sessions to work together

Researched November 2026 against Claude Code 2.1.323 and Herdr 0.9.5. The
question: how can several agent sessions on one card talk to each other and
work on a common problem?

Claims are marked **documented** (Claude Code or Herdr docs), **seen** (looked
at on this machine), or **unverified**.

Updated November 13, 2026 against the
[agent-swarm research](../reports/Agent%20swarm%20orchestration%20patterns.md).
This update refines design implications; it does not reverify external API
claims or approve implementation. Section 5 supersedes the initial Claude-only
recommendation; section 6 adds the reliability requirements.

## Four ways to do it

| Route | Who talks | Works for | Starfleet effort |
| --- | --- | --- | --- |
| Claude Code cross-session messaging | agent → agent, directly | Claude Code only | small: name sessions, tell them who else is on the card |
| Claude Code agent teams | lead → its own teammates | Claude Code only, inside one session | none, but it bypasses Starfleet |
| Starfleet as a relay | Starfleet reads one pane, prompts another | any agent CLI | medium: a relay rule engine |
| A shared file on the card | agents read and write a file | any agent CLI | small, but nobody gets woken up |

These routes can combine. The initial recommendation below is retained for
context; section 5 replaces it with a cross-harness proposal.

## 1. Claude Code cross-session messaging

Built into Claude Code since v2.1.224 and on by default. (documented)

- Two tools: `ListAgents` finds reachable sessions; `SendMessage` sends
  plain text to one by name. The receiver can reply with `SendMessage` to
  the sender's `from` address. (documented)
- A session's address is its name, set with `--name` / `-n` or `/rename`;
  otherwise Claude Code picks one. When names collide, a short `[ref]` is
  added. (documented)
- Delivery on the same machine uses a per-session Unix socket, never
  Anthropic's servers. Here they live in `/run/user/1000/cc-socks/<pid>.sock`,
  one per Claude process, and the path is exported to the session as
  `CLAUDE_CODE_MESSAGING_SOCKET` with a token in
  `CLAUDE_CODE_MESSAGING_TOKEN`. (documented; path seen)
- A busy receiver reads the message between tool calls; an idle one starts
  a new turn with it. So a message wakes an idle agent. (documented)
- `SendMessage` with `notify_when_idle: true` asks a session on this
  machine for one notice when it next goes idle or exits. This is the
  "tell me when you are done" primitive, with no polling. (documented)
- `claude -p` workers also get an inbox; `--bare` sessions do not.
  (documented)
- Sessions on other machines and cloud sessions are reachable through
  Remote Control. Not needed for Starfleet, whose agents all run on one
  machine. (documented) A session inside WSL and a native Windows session
  cannot reach each other, which is fine: all Starfleet agents run in WSL.
  (documented)

### What gets in the way

- **Permission-mode classes.** With no `crossSessionInbound` setting, a
  session that bypasses permission prompts holds messages from prompting
  sessions for approval, and vice versa. Held messages expire after five
  minutes (`dialogExpiry`). Agents on a card should all run in the same
  class, or set `crossSessionInbound: "accept"` through `--settings` at
  launch. (documented)
- **No permission laundering.** A message cannot approve a prompt, change
  settings or `CLAUDE.md`, or run a slash command. An agent is told never to
  ask a peer to do what its own session was denied. (documented) This is a
  feature, but it means "the reviewer tells the implementer to push" still
  ends at a prompt in the implementer's pane.
- **Plain text only, about 1M characters max.** No files; send paths and
  let the receiver read them. (documented)
- **Loops are throttled.** Bursts are refused at the sender, identical
  repeats are dropped, and at most 50 messages queue. (documented) These
  controls do not establish a bound on total exchanges, runtime or spend.
- **Every delivered message costs tokens** like a typed prompt. (documented)
- **OpenCode sessions are left out.** They have no inbox.

### Already seen on this machine

`ListAgents` from this session (which itself runs in Herdr pane `w3:p1`)
listed one other local interactive session, plus cloud and Remote Control
sessions. The session's own name was shown as the address peers would use.
No message was sent as part of this research.

## 2. Claude Code agent teams

One "lead" session spawns teammates that share a task list and mailboxes
(`~/.claude/teams/<team>/inboxes/<agent>.json`, tasks in
`~/.claude/tasks/<team>/`). Experimental, off unless
`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`. (documented)

Why it fits Starfleet poorly:

- The lead spawns and owns the teammates. Starfleet dispatches each session
  itself, so the sessions on a card are peers, not a team.
- Split panes need tmux or iTerm2; teammates would not appear as Herdr
  panes, so Starfleet cannot see them. The default in-process mode keeps
  them all inside one pane.
- One team per session, no nesting, teammates do not come back on resume.

It is still useful *inside* one station: a Picard station could be told to
run a team. Starfleet would see one session. Nothing to build.

## 3. Starfleet as the relay

Starfleet already has what it needs from Herdr: status events per pane,
`pane.read` for output, and `agent.prompt` to send text. (documented, and in
use) A relay rule would read: "when session A stops working, send its saved
output to session B with this instruction". Example: implementer done →
reviewer gets the tail and "review the changes on this branch"; reviewer
done → implementer gets the review.

- Works for any CLI Herdr can drive, OpenCode included.
- Deterministic and visible on the board; the user can see and stop it.
- `agent.prompt` is refused while the target is blocked
  (`agent_blocked` / `agent_not_ready`). Starfleet already holds a pending
  prompt in that case and could reuse that.
- This is the "Autonomy" item deferred in `03-decisions.md`: the app would
  act without a click. It needs that decision first.

Starfleet could also post into a Claude session's inbox socket directly. Not
recommended: the line format for a third party is undocumented beyond the
optional auth line, and Starfleet is not the session's child, so the
message is treated like any peer message. `agent.prompt` is documented.

## 4. A shared file on the card

A file in the card's folder (for example `.starfleet/mission.md`) that every
agent on the card is told to read before starting and append to when it
decides something. Works with every CLI, survives restarts, and leaves a
record. It wakes nobody, so it complements messaging rather than replacing
it. Keep it out of git or in `.gitignore`.

claude-mem is not a substitute: its notes are per project and written after
the fact by its worker, not something agents address to each other.

## Initial recommendation (superseded by section 5)

**Now (small change):** make Claude Code's own messaging usable on a card.

1. In `dispatch` (`crates/core/src/app.rs`), when the kind is `claude`, add
   `--name <name>` to the args, using the name already given to Herdr
   (`riker-ab12`). The Herdr name and the Claude Code address then match,
   and the board can show it.
2. Add a "crew" paragraph to the first message: the other live sessions on
   the card, each with its name, station and duty, and a line saying they
   can message each other with `SendMessage` and should report decisions to
   whoever is waiting on them.
3. Launch every agent on a card in the same permission class, or pass
   `--settings '{"crossSessionInbound":"accept"}'` for card sessions, so
   messages are not held for five minutes and dropped.
4. Optionally create `.starfleet/mission.md` and name it in the crew
   paragraph.

Sessions dispatched later are not known to earlier ones. Either tell earlier
sessions about a newcomer with a short `agent.prompt` (a click-driven
action, so no autonomy decision needed), or rely on agents running
`ListAgents`, which lists every local session.

**Later, in this initial proposal:** relay rules (route 3) once the autonomy
question is settled. Section 5 replaces this with cross-harness comms.

## 5. Across harnesses: picking the right tool and still talking

The real need: a Claude Code session working on a card realises it needs an
image, and the right tool for that is OpenCode with an OpenAI model. Today
the user has to say "go do that through OpenCode". The goal is for the
agent to ask for it itself, get the result back, and carry on, with any
Herdr-supported harness on either end.

### What Herdr already gives every agent

Every Herdr pane gets `HERDR_ENV=1`, `HERDR_PANE_ID` and
`HERDR_SOCKET_PATH`. (seen) The `herdr` CLI works from inside a pane, and
Herdr ships a skill (`herdr --skill`, or `npx skills add herdrdev/herdr
--skill herdr -g`) that teaches an agent to start helper agents in sibling
panes, `herdr agent prompt <name> "…" --wait`, `herdr agent wait` and
`herdr agent read`. (documented) Herdr drives more than 25 harnesses this
way: Claude Code, Codex, OpenCode, Gemini CLI, Copilot CLI, Cursor, Pi and
others. (documented)

So the transport between harnesses exists and is harness-neutral. On its
own it is not enough:

- The target gets the text as if the user typed it: no sender, no reply
  address, nothing saying it came from another agent.
- `--wait` blocks the caller's tool call, capped at 300 s. An image or a
  long review takes longer.
- The caller has to know which kind and model to start, and with what
  arguments. That knowledge is in Starfleet's roster, not in the agent.
- Starfleet sees the helper only as an unassigned contact, not as part of
  the card.

### Design: Starfleet comms, an MCP server every harness loads

Claude Code, OpenCode and Codex all load MCP servers. (documented) An MCP
server is the one extension point they share, so the collaboration layer
goes there, not into each harness.

```
 Claude Code ─┐                       ┌─ agent.start / agent.prompt ─┐
 OpenCode ────┼─ stdio MCP: starfleet ├─ app socket ─ Starfleet core ┤ Herdr
 Codex ───────┘   comms (per agent)   └─ board: crew, log, tickets ──┘
```

- **One binary.** `starfleet comms` runs as a stdio MCP server, started by
  the harness. It inherits the pane's environment (OpenCode documents this;
  unverified for the others), so `HERDR_PANE_ID` tells it which session is
  calling. It forwards each call to the running Starfleet app over a local
  socket (for example `$XDG_RUNTIME_DIR/starfleet/comms.sock`).
- **Installed once, globally, per harness.** Outside a Starfleet-managed
  pane (no `HERDR_PANE_ID`, or a pane not on any card) the server offers no
  collaboration tools. MCP startup/configuration overhead still needs measuring.
- **Starfleet routes and records.** Routing, queueing and permission live
  in the core, where the board is. Every message goes into the card's log,
  so the user sees the conversation.

Tools the server offers:

| Tool | Does |
| --- | --- |
| `crew()` | sessions on my card: name, station, harness, model, status |
| `stations(capability?)` | stations that can be called in, by capability |
| `send(to, text)` | message a crew member |
| `request(station or capability, prompt)` | start a helper on this card; returns a ticket at once |
| `reply(text, files?)` | answer the message or request that started me |

### Delivery

Starfleet delivers a message with Herdr's `agent.prompt`, so it works for
every harness, framed so the receiver knows what it is:

```
[Subspace from riker-ab12 · Claude Code · card "Sector sprites"]
<text>
Answer with the starfleet comms `reply` tool, not in chat.
```

- Target `idle` or `done`: deliver now.
- Target `working`: queue and deliver when Herdr reports it settled.
  Whether a harness accepts typed input mid-turn differs per harness and
  `agent.prompt` may answer `agent_prompt_stalled`, so queuing is the safe
  default. (unverified per harness)
- Target `blocked`: hold, and show it on the board, since the user has to
  answer the menu first.

Claude Code to Claude Code can also use `SendMessage`, but that bypasses the
proposed comms log and controls. Whether managed collaboration may use this
path is an open policy decision.

### Calling in another harness: the image case

1. Roster gains a `capabilities` field per station. A new station, say the
   Holodeck (location: Enterprise-D), has `agent: opencode`, an OpenAI
   model, a brief about producing images to a given path, and
   `capabilities: ["image"]`.
2. Riker (Claude Code) calls `request("image", "112×112 pixel-art DS9 per
   art/sites/STYLE.txt, write art/sites/ds9.png")`. The tool returns a
   ticket straight away; Riker keeps working or ends its turn.
3. Starfleet dispatches the Holodeck on the same card, in a new tab, with
   the brief, the request, and the requester's name.
4. The Holodeck replies with the artifact, outcome and evidence. If it stops
   without replying, Starfleet marks the request as missing a result and makes
   its saved output available for diagnosis, not as a successful reply.
5. Starfleet delivers the reply to Riker as a message, which wakes it if
   idle. The ticket records response delivery, not verification, acceptance or
   integration. Those are separate states and decisions.

The same path covers "second opinion from GPT on this diff", "have Codex
review", or a Claude reviewer for an OpenCode implementer.

### Launch adapters per harness

Starting each harness right becomes a small table in the core, keyed by
Herdr kind, instead of hand-written arguments in the roster:

| Kind | Name / address | Model flag | Loads comms via | Peer messages marked as |
| --- | --- | --- | --- | --- |
| `claude` | `--name` | `--model` | global MCP config, or `--mcp-config` | from another session (native) |
| `opencode` | Herdr name only | model in args/config | global `opencode mcp add --global` | Starfleet framing only |
| `codex` | Herdr name only | `--model` | global `mcp_servers` in config | Starfleet framing only |

Rows other than `claude` need checking against each CLI. (unverified)

### Guard rails

This is agents acting without a click, the "Autonomy" item deferred in
`03-decisions.md`, so it needs limits that the user sets per card:

- **Reach**: only sessions on the same card; never unassigned panes or
  other cards.
- **Requests**: a card setting of off, ask (a prompt on the board), or
  allowed for named stations only. Default: ask.
- **Budget**: a cap on messages and helper sessions per card per hour,
  plus a hop limit on reply chains. Traffic limits do not bound tokens, spend,
  runtime or concurrent execution; those need separate controls.
- **No permission laundering**: a helper is never started in a looser
  permission mode than the session that asked for it. A Claude session
  that would be prompted before running a command must not get the same
  command run by an OpenCode helper that auto-approves. Stations carry
  their permission posture, and `request` refuses a looser one. Cross-harness
  comparison must cover actual tools, credentials and resource access, not
  permission-mode names. Unknown equivalence requires approval or refusal.
- **Messages arrive as user input** in harnesses without a native peer
  concept, so the framing and the board log are the only markers. Treat
  this as the main risk of the design. Peer text must not gain user authority;
  framing and logs alone cannot enforce that boundary.

## Recommendation, revised

1. **Try it by hand first, nothing built.** Install the Herdr skill for
   Claude Code and OpenCode and run the image case with plain
   `herdr agent start/prompt/read` in a card's workspace. This shows how
   each harness behaves when prompted by another agent before any code
   depends on it.
2. **Name sessions and add the crew paragraph** (section above): small, and
   useful on its own for Claude-only cards.
3. **Define the request/result contract and approval policy**, then propose
   `starfleet comms` with `crew`, `send`, `reply` and queued delivery. Add
   `stations` and `request` with durable tickets and execution controls. This
   is a candidate sequence, not build authorization.

## 6. Messaging is not reliable delegation

The [swarm research](../reports/Agent%20swarm%20orchestration%20patterns.md)
supports the transport direction but adds requirements messaging alone cannot
satisfy. These are recommendations, not implemented Starfleet guarantees.

- **Ownership:** the requester owns continuation. Same-card membership does
  not assign responsibility for an objective. A permanent orchestrator is not
  necessary for the first bounded exchange.
- **Contract:** persist task, parent and attempt identity, objective, exclusions,
  context, expected artifacts, acceptance criteria and limits. Results include
  an outcome, artifacts, checks/results, deviations, blockers and risks. The
  tool signatures above are sketches, not a finalized protocol.
- **Correlation:** replies identify their request explicitly. A session may
  receive multiple requests; "the request that started me" is insufficient.
- **Lifecycle:** distinguish execution stopped, result submitted, response
  delivered, verified, accepted and integrated. Represent blocked, failed,
  canceled, expired and exhausted attempts. A truncated tail, possibly captured
  while blocked, is not completion evidence.
- **Recovery:** persist requests, queued messages and attempts. Reconcile live
  sessions and uncertain delivery after restart before retrying side effects.
  Detect abandoned helpers and expired deadlines. Cancellation does not undo
  effects or prove a process stopped.
- **Limits:** enforce deadlines, concurrency, retries, descendant-wide limits
  and cancellation outside prompts. Check available usage telemetry before
  promising hard spend caps.
- **Workspace safety:** current sessions share the card's folder. Concurrent
  code writers need separate worktrees/clones, recorded base revisions and
  controlled integration. Worktrees do not isolate credentials or services.
- **Evidence:** inspect actual artifacts against requirements, not just author
  explanations. Bind checks and approvals to revisions or hashes; relevant
  changes invalidate verdicts. Verification is not automatic user acceptance.
- **Enforcement boundary:** direct Herdr launches and native peer messages can
  bypass comms. Specify guarantees for managed requests and which underlying
  capabilities must be restricted for stronger guarantees.

Define the contract and manually test one exchange before polished agent
instructions. Build the minimum reliable loop before autonomous use; add writer
isolation and revision-bound verification before concurrent coding. Keep broader
hierarchy and automatic acceptance deferred. Proposed capabilities and open
decisions are tracked in [the feature inventory](../docs/features.md).

## To verify before building

1. That `--name` passed through `agent.start` args reaches Claude Code and
   becomes its `ListAgents` address. (unverified)
2. That two Herdr-launched sessions in the same mode deliver to each other
   without a hold, and that an idle receiver starts a turn. (documented,
   untested here)
3. How the Herdr status of a receiving pane moves when a message wakes it
   (`idle` → `working`), so the board reflects agent-to-agent activity.
   (unverified)
4. Whether `notify_when_idle` is reliable enough that agents can wait on
   each other without the user. (unverified)
5. That OpenCode and Codex pass `HERDR_PANE_ID` through to a stdio MCP
   server they start. (documented for OpenCode's environment, untested)
6. What `agent.prompt` does to a harness that is already `working`:
   queued input, interrupted turn, or `agent_prompt_stalled`. Per harness.
   (unverified)
7. Whether OpenCode and Codex show a clear "this came from another agent"
   marker anywhere, or only the framing text. (unverified)

## Sources

- https://code.claude.com/docs/en/cross-session-messaging
- https://code.claude.com/docs/en/agent-teams
- https://herdr.dev/docs/agents/
- https://herdr.dev/docs/agent-automation/
- https://herdr.dev/docs/agent-skill/
- https://opencode.ai/docs/mcp-servers/
- `research/01-herdr.md` for the Herdr methods named here
