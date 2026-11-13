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
- The map stays plain SVG. The pixel-art sprites for the four locations
  (and the Earth backdrop) are `<image>` elements drawn with
  `image-rendering: pixelated`, so a sprite engine (Phaser or PixiJS) has not
  been needed. Revisit it only if the map gains animation or many more
  objects.

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

This is the tail only, with the agent UI's own chrome removed (live view and
saved copy alike). Long box-drawing rules are dropped, and for Claude Code
everything from its last status line onward (`✻ Churned for 3m 22s`, or
`✶ Frolicking… (50s · …)` while it works) is cut: the recap, input box and
footer. Output with no such line, for example from OpenCode, is kept whole.
The status-line match follows Claude Code's current UI, so a change there
would let the extra chrome back in rather than lose content. Output written
after the agent's last stop is not captured if the pane is closed before it
stops again.

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

### Installing: a local script, not published binaries

`tools/install.sh` builds a release binary from a checkout and puts it in
`~/.local/bin`, with a menu entry and icon on Linux. The menu entry names the
icon by absolute path and sets `StartupWMClass`; a bare theme name was not
resolved by the taskbar. Started from a terminal, a release build detaches
itself (`setsid`) so the prompt returns; `--foreground` keeps it attached.

A tag-triggered release workflow (`.github/workflows/release.yml`) can build
Linux, Windows and macOS bundles into a draft GitHub release. It has never
run, and the bundles would be unsigned, so it is not the install path yet. A
curl-able install script was passed over: the platform installers cover that
job, and from inside WSL a script cannot install the Windows app.

### The repository is public

It is a personal tool with no plans to distribute, but the repo is public
(for GitHub runners). Machine settings and the board file are ignored by
git. Star Trek names belong to Paramount; this matters only if builds are
ever distributed.

## Deferred

- **More than the tail on a card**: a summary (possibly from claude-mem), a
  file, a PR link. The cleaned tail is saved today.
- **Autonomy**: whether the app ever acts on its own (answering a blocked
  agent, choosing an agent). For now it only acts on a click.
- **Where unassigned sessions sit in the UI**: currently a side tray on the
  board; to be revisited after using it.
- **Better pixel art**: the sprites are in, generated by agents from the
  scripts and shared style guide in `art/sites/`. They are placeholder
  quality: the Enterprise-D lacks its nacelle pylons and Deep Space Nine's
  docking pylons are uneven. Regenerate them from those scripts. Hover
  behaviour (highlight, 5% larger) is in place on the sprites.
- **Sync**: claude-mem cards, an Obsidian vault, or a home Kubernetes
  cluster, behind a plugin interface.
- **Cross-machine view**: Herdr has SSH machine federation if this is ever
  wanted.
