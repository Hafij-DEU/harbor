#!/bin/bash
# Render a Harbor app window to PNG in a private headless GNOME Shell.
#
#   tools/shot.sh APP OUT.png [--size 1200x780] [--theme dark|light]
#                 [--do 'python statement' ...] [--delay MS] [--overlay EXPR]
#
# HB_FILES_JSON=settings.json seeds the Files settings for the run.
#
# APP is files, settings or taskmgr. Each --do runs in order (globals: win,
# app, Gtk, GLib, Adw) with --delay ms between them, then the window is
# rendered. The app runs with HARBOR_DRY_RUN=1 and an in-memory GSettings
# backend, so nothing on the real system changes.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BASE=/tmp/claude-1000
[ -d "$BASE" ] && [ -w "$BASE" ] || BASE=${TMPDIR:-/tmp}
# Keep this short: the Wayland socket path is limited to 108 bytes.
RT=$(mktemp -d "$BASE/hb.XXXX")
mkdir -p "$RT/c" "$RT/d" "$RT/k" "$RT/r"
chmod 700 "$RT/r"
# Same XDG user folders as the real session (read-only copy), so the Home
# and This PC pages show the real folder list.
[ -f "$HOME/.config/user-dirs.dirs" ] && cp "$HOME/.config/user-dirs.dirs" "$RT/c/"
# The user's own fonts (fontconfig looks in $XDG_DATA_HOME/fonts).
[ -d "$HOME/.local/share/fonts" ] && ln -s "$HOME/.local/share/fonts" "$RT/d/fonts"
# HB_FILES_JSON=path seeds Harbor Files' settings for this run.
if [ -n "${HB_FILES_JSON:-}" ]; then mkdir -p "$RT/c/harbor"; cp "$HB_FILES_JSON" "$RT/c/harbor/files.json"; fi
# Export the private XDG dirs BEFORE dbus-run-session: dconf-service is
# D-Bus activated and inherits dbus-daemon's environment.
export XDG_CONFIG_HOME=$RT/c XDG_DATA_HOME=$RT/d XDG_CACHE_HOME=$RT/k XDG_RUNTIME_DIR=$RT/r
export HB_RT=$RT HB_ROOT=$ROOT HARBOR_DRY_RUN=1
unset DBUS_SESSION_BUS_ADDRESS WAYLAND_DISPLAY DISPLAY
set +e
dbus-run-session -- "$ROOT/tools/shot-inner.sh" "$@"
rc=$?
fusermount3 -uz "$RT/r/doc" 2>/dev/null; fusermount3 -uz "$RT/r/gvfs" 2>/dev/null
rm -rf "$RT" 2>/dev/null
exit $rc
