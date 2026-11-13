#!/usr/bin/env bash
# Build Starfleet from this checkout and install it for the current user.
#
#   tools/install.sh              build and install
#   tools/install.sh --uninstall  remove what it installed
#
# Installs `starfleet` into ~/.local/bin. On Linux it also adds an
# application-menu entry and icon. Set BIN_DIR to install somewhere else.
# Re-run after `git pull` to update.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN="$BIN_DIR/starfleet"
DESKTOP="$DATA_DIR/applications/starfleet.desktop"
ICON="$DATA_DIR/icons/hicolor/128x128/apps/starfleet.png"

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# rustup installs cargo outside the default PATH of non-login shells.
if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi

uninstall() {
  rm -f "$BIN" "$DESKTOP" "$ICON"
  say "Removed Starfleet from $BIN_DIR and the application menu."
  say "Your board and settings are untouched (see README, \"Files it keeps\")."
}

check_prerequisites() {
  local missing=()
  command -v cargo >/dev/null 2>&1 || missing+=("cargo (install Rust: https://rustup.rs)")
  command -v node >/dev/null 2>&1 || missing+=("node 20+ (https://nodejs.org)")
  command -v npm >/dev/null 2>&1 || missing+=("npm")
  if [ "$(uname -s)" = Linux ]; then
    if ! command -v pkg-config >/dev/null 2>&1 || ! pkg-config --exists webkit2gtk-4.1; then
      missing+=("webkit2gtk-4.1 dev files (Debian/Ubuntu: sudo apt install libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev)")
    fi
  fi
  if [ "${#missing[@]}" -gt 0 ]; then
    printf 'Missing prerequisites:\n' >&2
    printf '  - %s\n' "${missing[@]}" >&2
    say "See https://tauri.app/start/prerequisites/ for your OS." >&2
    exit 1
  fi
}

install_desktop_entry() {
  [ "$(uname -s)" = Linux ] || return 0
  mkdir -p "$(dirname "$DESKTOP")" "$(dirname "$ICON")"
  cp "$REPO/src-tauri/icons/128x128.png" "$ICON"
  cat >"$DESKTOP" <<EOF
[Desktop Entry]
Type=Application
Name=Starfleet
Comment=Control room for AI coding agents running in Herdr
Exec=$BIN
Icon=starfleet
Terminal=false
Categories=Development;
EOF
}

case "${1:-}" in
  "") ;;
  --uninstall) uninstall; exit 0 ;;
  -h | --help) sed -n '2,10p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
  *) die "unknown option: $1 (try --help)" ;;
esac

check_prerequisites

cd "$REPO"
say "Building Starfleet (the first build takes a few minutes)..."
npm ci
npm run tauri build -- --no-bundle

BUILT="$REPO/target/release/starfleet"
[ -x "$BUILT" ] || die "build finished but $BUILT is missing"

mkdir -p "$BIN_DIR"
# Write to a temp name and rename, so a running copy is replaced safely.
install -m 755 "$BUILT" "$BIN.new"
mv -f "$BIN.new" "$BIN"
install_desktop_entry

say "Installed $BIN"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) say "Note: $BIN_DIR is not on your PATH. Add it to your shell profile." ;;
esac
if [ "$(uname -s)" = Linux ]; then
  say "Added Starfleet to your application menu."
fi
