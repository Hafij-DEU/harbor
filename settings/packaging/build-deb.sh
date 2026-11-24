#!/usr/bin/env bash
set -euo pipefail

# Build a self-contained Debian package from the Cargo release binary.
# It intentionally installs to /usr/local so it never replaces Ubuntu's
# distribution-owned Settings application.

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
ARCH=$(dpkg --print-architecture)
PACKAGE="win11-settings"
OUT_DIR="$ROOT/dist"
OUT_FILE="$OUT_DIR/${PACKAGE}_${VERSION}_${ARCH}.deb"
STAGE=$(mktemp -d "${TMPDIR:-/tmp}/${PACKAGE}-deb.XXXXXX")
chmod 755 "$STAGE"

cleanup() {
    rm -rf "$STAGE"
}
trap cleanup EXIT

cargo build --release

mkdir -p \
    "$STAGE/DEBIAN" \
    "$STAGE/usr/local/bin" \
    "$STAGE/usr/share/applications" \
    "$STAGE/usr/share/icons/hicolor/scalable/apps" \
    "$STAGE/usr/local/lib/win11-settings" \
    "$STAGE/usr/lib/systemd/system" \
    "$STAGE/usr/share/polkit-1/actions"

install -m 0755 target/release/win11-settings "$STAGE/usr/local/bin/win11-settings"
# Keyboard Manager: root service, its pkexec helper and polkit action
# (how they fit together: src/keyboard/mod.rs).
install -m 0755 target/release/harbor-keyboardd "$STAGE/usr/local/lib/win11-settings/harbor-keyboardd"
install -m 0755 packaging/keyboard/harbor-keyboard-apply \
    "$STAGE/usr/local/lib/win11-settings/harbor-keyboard-apply"
install -m 0644 packaging/keyboard/harbor-keyboardd.service \
    "$STAGE/usr/lib/systemd/system/harbor-keyboardd.service"
install -m 0644 packaging/keyboard/io.github.hafij_deu.Harbor.Keyboard.policy \
    "$STAGE/usr/share/polkit-1/actions/io.github.hafij_deu.Harbor.Keyboard.policy"
install -m 0644 win11-settings.desktop \
    "$STAGE/usr/share/applications/win11-settings.desktop"
install -m 0644 assets/win11-settings.svg \
    "$STAGE/usr/share/icons/hicolor/scalable/apps/win11-settings.svg"

cat > "$STAGE/DEBIAN/control" <<EOF
Package: ${PACKAGE}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Maintainer: Hafij
Depends: libfontconfig1, libgl1, libegl1, libwayland-client0, libx11-6, libxkbcommon0, python3, python3-gi, dconf-cli, libglib2.0-bin
Recommends: network-manager, bluez, power-profiles-daemon, systemd-timesyncd, unattended-upgrades
Description: Windows 11 and Windows 10 style Settings for Ubuntu
 A Rust Settings application with switchable Windows 11 and Windows 10
 interfaces, Ubuntu system controls and integrated app management
 (standalone; App Hub catalogue is optional).
EOF

cat > "$STAGE/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
if command -v systemctl >/dev/null 2>&1; then
    # The old remapping service (0.5.35 and older) must stop before its unit
    # file is forgotten by the daemon-reload below.
    systemctl stop harbor-keymapd.service 2>/dev/null || true
    systemctl daemon-reload || true
    # 0.5.36 renamed harbor-keymapd to harbor-keyboardd and keymap.conf to
    # keyboard.conf: move an old setup over.
    if [ -e /etc/systemd/system/multi-user.target.wants/harbor-keymapd.service ]; then
        systemctl disable --now harbor-keymapd.service 2>/dev/null || true
        rm -f /etc/systemd/system/multi-user.target.wants/harbor-keymapd.service
    fi
    if [ -f /etc/harbor/keymap.conf ] && [ ! -f /etc/harbor/keyboard.conf ]; then
        sed 's/^enabled true$/remap on/; s/^enabled false$/remap off/' \
            /etc/harbor/keymap.conf > /etc/harbor/keyboard.conf
        rm -f /etc/harbor/keymap.conf
    fi
    # The service runs once the user has saved any keyboard setting.
    if [ -f /etc/harbor/keyboard.conf ]; then
        systemctl enable harbor-keyboardd.service >/dev/null 2>&1 || true
        systemctl restart harbor-keyboardd.service || true
    fi
fi
EOF
chmod 0755 "$STAGE/DEBIAN/postinst"

cat > "$STAGE/DEBIAN/prerm" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = remove ] && command -v systemctl >/dev/null 2>&1; then
    systemctl disable --now harbor-keyboardd.service || true
fi
EOF
chmod 0755 "$STAGE/DEBIAN/prerm"

cat > "$STAGE/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
if [ "$1" = remove ] || [ "$1" = purge ]; then
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database -q /usr/share/applications || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
    fi
fi
EOF
chmod 0755 "$STAGE/DEBIAN/postrm"

mkdir -p "$OUT_DIR"
rm -f "$OUT_FILE"
dpkg-deb --root-owner-group --build "$STAGE" "$OUT_FILE"
echo "Built $OUT_FILE"
