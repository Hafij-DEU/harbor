#!/bin/bash
# Runs inside dbus-run-session (see shot.sh).
set -u
gnome-shell --headless --virtual-monitor 1680x1050 --no-x11 --wayland-display hb-0 \
    >"$HB_RT/shell.log" 2>&1 &
SHELL_PID=$!
for _ in $(seq 100); do
    [ -S "$XDG_RUNTIME_DIR/hb-0" ] && break
    sleep 0.1
done
if [ ! -S "$XDG_RUNTIME_DIR/hb-0" ]; then
    echo "gnome-shell did not start:" >&2
    tail -20 "$HB_RT/shell.log" >&2
    kill "$SHELL_PID" 2>/dev/null
    exit 3
fi
WAYLAND_DISPLAY=hb-0 GDK_BACKEND=wayland GSETTINGS_BACKEND=memory GSK_RENDERER=cairo \
    timeout 60 python3 "$HB_ROOT/tools/drive.py" "$@"
rc=$?
kill "$SHELL_PID" 2>/dev/null
wait "$SHELL_PID" 2>/dev/null
exit $rc
