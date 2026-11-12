# Starfleet

A Starfleet-themed control room for AI coding agents running in
[Herdr](https://herdr.dev). Pick a location, pick a station, write a
prompt, and an agent starts in a new Herdr pane. A board of cards tracks
the work and shows which agents need you.

This is a personal tool and an early first version.

## What it does

- **Sector map**: Starfleet Academy, the Daystrom Institute, the
  Enterprise-D and Deep Space Nine, as wireframes. Each holds stations.
- **Stations**: presets for an agent (CLI, model, standing brief). Clicking
  one dispatches an agent for a card.
- **Mission board**: cards, each tied to a working folder, each with any
  number of sessions and their live status. Agents you started by hand in
  Herdr show up as unassigned and can be attached to a card.
- **Session view**: live output tail, follow-up message, reply keys for a
  blocked agent, and a button that brings the pane to the front in Herdr.
  When an agent stops working its output is saved on the card, so a closed
  session still shows what it said.
- **System check**: verifies Herdr, Claude Code, OpenCode and claude-mem in
  the environment where the agents run (WSL, when on Windows).

## Requirements

- [Herdr](https://herdr.dev) running, and Claude Code installed.
- Optional: OpenCode, claude-mem.
- To build: Rust, Node 20+, and the
  [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS.
- On Windows: WSL, with Herdr and the agent CLIs installed inside it, plus
  one of `socat`, `python3` or `nc` there.

## Run it

```bash
npm install
```

```bash
npm run tauri dev
```

Build a release binary:

```bash
npm run tauri build
```

### Try it without Herdr

A fake Herdr server is included for demos and tests:

```bash
cargo run -p mock-herdr -- /tmp/mock-herdr.sock
```

```bash
HERDR_SOCKET_PATH=/tmp/mock-herdr.sock STARFLEET_HOME=/tmp/starfleet-demo npm run tauri dev
```

`npm run dev` alone serves the UI in a browser on simulated data.

## Files it keeps

| File | Linux and macOS | Windows |
| --- | --- | --- |
| `settings.json`, `roster.json` | `~/.config/starfleet/` | `%APPDATA%\starfleet\` |
| `board.json` | `~/.local/share/starfleet/` | `%LOCALAPPDATA%\starfleet\` |

`XDG_CONFIG_HOME` and `XDG_DATA_HOME` are honoured. `STARFLEET_HOME` puts
everything under one folder.

- **`roster.json`** is created on first run. Edit it to change stations,
  their agent, model, arguments and standing brief.
- **`settings.json`** is optional:

  ```json
  {
    "herdr_socket": null,
    "herdr_session": null,
    "wsl_distro": null
  }
  ```

## Layout

- `crates/core`: the logic (Herdr client, board, dispatch, system check).
- `crates/mock-herdr`: the fake Herdr server.
- `src-tauri`: the desktop shell.
- `src`: the React front end.
- `research/`: notes from the initial research and the decisions made.

## Tests

```bash
cargo test --workspace
```

## Status

Built and tested on Linux (WSL, then Ubuntu) against the fake Herdr server,
which was written from Herdr's published API schema. The core has also been
run against a real Herdr 0.9.5 on Ubuntu: dispatch, prompt delivery and
status tracking work. It has not yet been run on native Windows or on macOS.
`research/01-herdr.md` lists what was checked and what still needs checking.
