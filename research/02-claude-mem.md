# claude-mem: what it offers and where it fits

Researched November 2026 from the claude-mem docs. Not tested against a
running install.

## What it is

A Claude Code plugin that gives sessions persistent memory. Hooks capture
session events, a local background worker processes them, and everything is
stored in SQLite.

- Data directory: `~/.claude-mem` (override with `CLAUDE_MEM_DATA_DIR`).
- Database: `~/.claude-mem/claude-mem.db`.
- Worker: a local HTTP service. Health check at `/api/health`.

## The worker port

The docs disagree with each other, so do not hardcode a port.

- Current configuration docs: default is **`37700 + (uid % 100)`**, so
  37700 for a typical UID of 1000.
- Older docs, and one current page, say 37777.
- `CLAUDE_MEM_WORKER_PORT` overrides it.

Starfleet's system check resolves the port in this order: the
`CLAUDE_MEM_WORKER_PORT` environment variable, then that key in
`~/.claude-mem/settings.json`, then `37700 + (uid % 100)`. It then calls
`/api/health`.

## My Focus, My Work, My Rules

claude-mem's viewer now has its own board-like features:

- **My Focus**: focus areas it derives from your prompts.
- **My Work**: cards for pieces of work, with status and deliverables.
- **My Rules**: rules it proposes, which you approve or reject.

These are generated automatically. The gap that matters for this project:
there is no documented way to tell it that a new session belongs to a card
it already made. The docs mention `work_state_read` and `work_state_write`
MCP tools, but nothing that links a session to an existing card.

## Consequence for Starfleet

- Starfleet owns its own board. Cards are created by the user, and sessions
  are attached to cards by the user.
- claude-mem is optional. The system check reports it as a warning, never a
  failure, when it is missing.
- Not built yet: reading session summaries from claude-mem onto cards.
  Before designing that, check the installed version's actual HTTP
  endpoints, since the docs list 10, 14 and 22 endpoints in different
  places.
- Possible later: sync Starfleet cards with claude-mem's cards (it has a
  paid cloud sync), or with an Obsidian vault, behind a plugin interface.

## Sources

- https://docs.claude-mem.ai/configuration
- https://docs.claude-mem.ai/usage/my-work
- https://docs.claude-mem.ai/architecture/worker-service
- https://github.com/thedotmack/claude-mem
