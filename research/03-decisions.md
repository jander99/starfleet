# Design decisions and open questions

A record of what was decided during the initial research, and why.

## The idea

A desktop app styled as a 2D Starfleet game that is really a control room
for AI agents running in Herdr. Pick a place (Starfleet Academy, the
Daystrom Institute, the Enterprise-D, Deep Space Nine), pick a station
there, give it a prompt, and an agent starts in a new Herdr pane. A board
of cards tracks the work; each card can have several sessions.

A friend's Pokémon-themed dashboard was the reference point. It is
read-only and polls session files. Starfleet differs in two ways: it
dispatches work as well as watching it, and it gets status pushed from
Herdr events.

## Decisions

### UI framework: Tauri 2 with a web front end

One codebase builds for Windows, Linux and macOS. The Rust backend suits a
socket client and process management. The front end is React and
TypeScript.

- Electron was the fallback if webview differences caused trouble.
- A game engine (Godot and similar) was rejected: forms, boards and text
  output are awkward there.
- The map is plain SVG for now, since the first art is wireframe. A sprite
  engine (Phaser or PixiJS) can replace the map view when pixel art exists.

### Windows reaches Herdr through a relay inside WSL

Herdr and the agent CLIs live in WSL. A Windows process cannot open a Unix
socket that lives inside WSL2, so the app starts a long-lived
`wsl.exe -e sh -c …` process that bridges the socket to its own stdin and
stdout, using whichever of `socat`, `python3` or `nc` is present. The API
is newline-delimited JSON, so it passes through a pipe unchanged.

The relay path is covered by an automated test, with a native shell
standing in for `wsl.exe`. It has not yet been run on real Windows.

### Do not rebuild the chat

Herdr already shows the full terminal. Starfleet shows a read-only tail of
the output, one follow-up box, quick reply keys for when an agent is
blocked on a menu, and a "jump to pane" button. Anything richer happens in
a terminal attached to Herdr.

### Starfleet owns the board

Cards live only in this app, in a local JSON file. The user creates cards
and decides which session belongs to which card. Agents started by hand in
Herdr appear as "unassigned contacts" and can be attached to a card. See
`02-claude-mem.md` for why this is not delegated to claude-mem.

### A card carries the working folder

Each card has a folder (a git repo, or any directory). Every session
dispatched for that card starts there. On Windows the folder is a path
inside WSL.

### One Herdr workspace per card

The first dispatch for a card creates a workspace and tags it with the
card's id. Later dispatches for the same card open a new tab in that
workspace.

### A finished session leaves its output on the card

Each time a session's agent stops working (done, idle, or blocked on a
question), Starfleet reads the last 200 lines of the pane and saves them on
the session's link in the board file, capped at 32 KB. When the pane is
later closed, the session drawer shows that saved output instead of nothing.
The board view only carries when it was saved; the text is fetched when the
drawer opens, to keep the view small.

This is the raw tail only. Output written after the agent's last stop is not
captured if the pane is closed before it stops again.

### Stations are presets

A station is: which agent CLI, an optional model, extra arguments, and a
standing brief. The brief is sent as the opening of the first message, so
it works the same for every agent CLI. The roster is a JSON file the user
can edit. The default roster uses the main cast of each show.

### Providers: two CLIs cover everything

- Claude Code for Claude (the main provider).
- OpenCode for ChatGPT (OAuth) and Gemini (API key).

Agent and model can be overridden per dispatch.

### App files follow XDG

- Linux and macOS: `$XDG_CONFIG_HOME/starfleet` (default
  `~/.config/starfleet`) and `$XDG_DATA_HOME/starfleet` (default
  `~/.local/share/starfleet`). macOS uses XDG on purpose.
- Windows: `%APPDATA%\starfleet` and `%LOCALAPPDATA%\starfleet`.

### Machines are independent

Each install has its own config and its own board. Work and personal
accounts never mix. No cross-machine sync in the first version.

### Build order

Linux first, then the WSL bridge on Windows, then macOS.

### The repository is public

It is a personal tool with no plans to distribute, but the repo is public
(for GitHub runners). Machine settings and the board file are ignored by
git. Star Trek names belong to Paramount; this matters only if builds are
ever distributed.

## Deferred

- **More than the raw tail on a card**: a summary (possibly from
  claude-mem), a file, a PR link. The raw tail is saved today.
- **Autonomy**: whether the app ever acts on its own (answering a blocked
  agent, choosing an agent). For now it only acts on a click.
- **Where unassigned sessions sit in the UI**: currently a side tray on the
  board; to be revisited after using it.
- **Pixel art**: generated later, possibly by an agent. Hover behaviour
  (highlight, 5% larger) is already in place on the wireframes.
- **Sync**: claude-mem cards, an Obsidian vault, or a home Kubernetes
  cluster, behind a plugin interface.
- **Cross-machine view**: Herdr has SSH machine federation if this is ever
  wanted.
