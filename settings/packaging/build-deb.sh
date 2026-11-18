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
    "$STAGE/usr/share/icons/hicolor/scalable/apps"

install -m 0755 target/release/win11-settings "$STAGE/usr/local/bin/win11-settings"
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
EOF
chmod 0755 "$STAGE/DEBIAN/postinst"

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
