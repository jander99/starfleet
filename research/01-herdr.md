# Herdr: what it offers and how Starfleet uses it

Researched November 2026 against Herdr 0.9.5 docs and the API schema for
protocol 22 (output of `herdr api schema --json`).

Each claim below is marked **documented** (stated in Herdr's docs or schema)
or **unverified** (our reading or assumption, not yet tested against a real
Herdr server).

## What Herdr is

A terminal multiplexer built for running several AI coding agents at once. A
single Rust binary. A background server owns the terminals; clients attach
and detach while agents keep running.

- Hierarchy: **workspace → tab → pane**. (documented)
- It recognises agent CLIs in panes and reports a semantic status for each:
  `idle`, `working`, `blocked`, `done`, `unknown`. `done` means the agent
  finished and nobody has looked at the pane yet. (documented)
- Detection combines the foreground process, terminal output patterns, and
  optional per-agent integrations that report state directly. (documented)
- There is **no pause or resume command**. What Herdr gives is persistence
  (detach, reattach) and the `blocked` state. (documented by absence: none in
  the CLI reference or schema)

## The socket API

- Transport: Unix domain socket; a named pipe on native Windows. (documented)
- Framing: newline-delimited JSON. Request `{"id","method","params"}`,
  reply `{"id","result"}` or `{"id","error":{"code","message"}}`. (documented)
- Socket path: `~/.config/herdr/herdr.sock`, or
  `~/.config/herdr/sessions/<name>/herdr.sock` for a named session.
  Resolution order: `--session`, `HERDR_SOCKET_PATH`, `HERDR_SESSION`,
  default. (documented)
- The CLI is a wrapper over the same socket. (documented)

### Methods Starfleet calls

| Purpose | Method | Notes |
| --- | --- | --- |
| Health | `ping` | returns version and protocol |
| Full state | `session.snapshot` | workspaces, tabs, panes, agents |
| New work area | `workspace.create`, `tab.create` | both return the new `root_pane` |
| Tag a workspace | `workspace.report_metadata` | free-form `tokens` map |
| Start an agent | `agent.start` | `name`, `kind`, `pane_id`, `args`, `timeout_ms` |
| Send a prompt | `agent.prompt` | refuses with `agent_blocked` if the agent is blocked |
| Read output | `pane.read` | `source: recent`, plain text |
| Answer a menu | `pane.send_text`, `pane.send_keys` | keys such as `enter`, `esc` |
| Bring to front | `workspace.focus`, `tab.focus`, `pane.focus` | |
| Clean up | `pane.close` | after a failed start |
| Live updates | `events.subscribe` | connection stays open |
| Integrations | `integration.list` | state per agent CLI |

### Events

- `events.subscribe` takes a list of subscriptions. Most are global
  (`pane.created`, `pane.closed`, `pane.updated`, `workspace.*`, `tab.*`).
  (documented)
- `pane.agent_status_changed` **requires a `pane_id`**: there is no
  "all panes" form. Starfleet therefore subscribes per pane and resubscribes
  whenever the set of panes changes. (documented in the schema)
- Events are not durable. A slow subscriber gets `events_lost` and must
  resubscribe and reconcile with `session.snapshot`. (documented)
- If a subscription names a pane that no longer exists, the whole request is
  rejected. (documented)

### Identity

- `pane_id` values look like `w1:p1`. The docs do not promise they are
  stable, and they read as positional. (unverified, treated as unstable)
- Every pane also has a `terminal_id`. (documented) We assume it identifies
  the terminal for the life of the server. (unverified)
- `agent_session` (agent, kind, value) is reported by integrations and is
  what lets an agent come back in the same session after a server restart.
  (documented)

Starfleet matches a stored session to a live pane by `terminal_id` first,
then `agent_session`, and only falls back to `pane_id` when it never knew
anything better.

## Agents

Integration targets in the schema: `claude`, `codex`, `opencode`, `copilot`,
`cursor`, `grok`, `qwen`, `kimi`, and others. (documented)

- **Claude Code**: supported, with an integration. (documented)
- **OpenCode**: supported; its plugin also reports state and session
  identity. This is the route to ChatGPT (OAuth) and Gemini (API key)
  without installing more CLIs. (documented)
- **Gemini CLI**: detected but listed as less tested. (documented)

## Platforms

- Linux and macOS: full support. (documented)
- Native Windows: supported, with gaps (some integrations unavailable,
  remote attach limited). (documented)
- WSL: Herdr runs inside WSL like on any Linux. (documented)

For this project Herdr and the agent CLIs live **inside WSL** on Windows,
because that is where Claude Code is installed. See
`03-decisions.md` for how the Windows app reaches it.

## Still to verify against a real Herdr server

These could not be tested while building the first version, because the
build sandbox did not allow running the Herdr binary.

1. Accepted values for `agent.start`'s `kind` (assumed to match integration
   target names such as `claude` and `opencode`).
2. Whether `agent.prompt` delivers multi-line text as one message. Station
   briefs are joined to the user's prompt with a blank line.
3. Whether `pane.updated` fires on every output change (which would make
   that subscription noisy; snapshots are throttled to one per 300 ms).
4. Whether workspace `tokens` set without a TTL persist, including across a
   server restart.
5. Whether `terminal_id` is stable for the life of the server.
6. Whether single digits sent with `pane.send_text` answer Claude Code's
   permission menus.

## Sources

- https://herdr.dev/docs/socket-api/
- https://herdr.dev/docs/cli-reference/
- https://herdr.dev/docs/agents/
- https://herdr.dev/docs/windows-beta/
- https://herdr.dev/docs/persistence-remote/
- https://herdr.dev/docs/install/
- https://github.com/herdrdev/herdr
