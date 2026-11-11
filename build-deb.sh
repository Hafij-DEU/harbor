#!/bin/bash
# Build dist/harbor_<version>_all.deb (Files, Settings and Task Manager).
set -euo pipefail
cd "$(dirname "$0")"
PKG=harbor
VERSION=$(sed -n 's/^VERSION = "\(.*\)"/\1/p' src/harbor/__init__.py)
STAGE=$(mktemp -d)       # dpkg rejects the 777 dirs of the NTFS checkout
chmod 755 "$STAGE"
trap 'rm -rf "$STAGE"' EXIT

# Ship only finished apps: each one has a launcher in bin/. Unfinished
# modules (files_*, settings_* while those apps are being built) stay out.
APPS=()
for b in bin/harbor-*; do APPS+=("${b#bin/harbor-}"); done
declare -A ID=([taskmgr]=TaskManager [files]=Files [settings]=Settings)
for f in src/harbor/__init__.py src/harbor/common.py; do
    install -Dm644 "$f" "$STAGE/usr/lib/harbor/harbor/$(basename "$f")"
done
for a in "${APPS[@]}"; do
    for f in src/harbor/"$a".py src/harbor/"$a"_*.py; do
        [ -f "$f" ] && install -Dm644 "$f" "$STAGE/usr/lib/harbor/harbor/$(basename "$f")"
    done
    install -Dm755 "bin/harbor-$a" "$STAGE/usr/bin/harbor-$a"
    n=io.github.hafij_deu.Harbor.${ID[$a]}
    install -Dm644 "data/$n.desktop" "$STAGE/usr/share/applications/$n.desktop"
    install -Dm644 "data/icons/$n.svg" "$STAGE/usr/share/icons/hicolor/scalable/apps/$n.svg"
done
if [[ " ${APPS[*]} " == *" taskmgr "* ]]; then
    # Memory Sweep page: root helper for swap / zram / zswap and its polkit action.
    # Own paths and action id, so it installs next to the memory-sweep package.
    install -Dm755 data/harbor-swap "$STAGE/usr/libexec/harbor/harbor-swap"
    install -Dm644 data/io.github.hafij_deu.Harbor.policy \
        "$STAGE/usr/share/polkit-1/actions/io.github.hafij_deu.Harbor.policy"
fi
echo "apps: ${APPS[*]}"
install -Dm644 LICENSE "$STAGE/usr/share/doc/$PKG/copyright"

mkdir -p "$STAGE/DEBIAN"
cat > "$STAGE/DEBIAN/control" <<CTL
Package: $PKG
Version: $VERSION
Section: gnome
Priority: optional
Architecture: all
Depends: python3 (>= 3.10), python3-gi, gir1.2-gtk-4.0, gir1.2-adw-1 (>= 1.7), gir1.2-glib-2.0
Recommends: pkexec, network-manager, zram-tools
Maintainer: Md. Hafijur Rahman <mdhafijur.rahman@ogr.deu.edu.tr>
Installed-Size: $(du -sk "$STAGE/usr" | cut -f1)
Description: Windows 11-style apps for GNOME
 Harbor is a set of GTK4/libadwaita apps with a Windows 11 look.
 .
 Harbor Task Manager: processes grouped by app with End task and
 Efficiency mode, live CPU / memory / swap / disk / network / GPU graphs,
 a Memory Sweep page (biggest apps, extra helper copies, files held in
 RAM in /tmp and /dev/shm, with suggestions to free memory), memory
 optimization settings for zram, swap file and zswap, a high-memory
 warning, app history, startup apps, users, details with priorities, and
 systemd services. Actions that need root ask through polkit.
 .
 Harbor Files: a File Explorer with tabs, a breadcrumb address bar,
 recursive search, Home and This PC pages, Quick access pins, drives
 with mount and eject, trash with restore, cut / copy / paste with
 progress, ZIP compress and safe extract, open with, a details pane and
 properties. Delete moves to the trash; permanent delete asks first.
CTL
cat > "$STAGE/DEBIAN/postinst" <<'SH'
#!/bin/sh
set -e
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q /usr/share/icons/hicolor || true
command -v update-desktop-database >/dev/null && update-desktop-database -q /usr/share/applications || true
SH
cp "$STAGE/DEBIAN/postinst" "$STAGE/DEBIAN/postrm"
chmod 755 "$STAGE/DEBIAN/postinst" "$STAGE/DEBIAN/postrm"

mkdir -p dist
dpkg-deb --root-owner-group --build "$STAGE" "dist/${PKG}_${VERSION}_all.deb"
