#!/bin/bash
# Build one .deb per app: dist/harbor-files_<v>_all.deb and
# dist/harbor-taskmgr_<v>_all.deb. Each app has its own version
# (src/harbor/__init__.py) and its own copy of the shared modules in
# /usr/lib/harbor-<app>, so the two packages never own the same file.
#   ./build-deb.sh            both apps
#   ./build-deb.sh files      only Harbor Files
set -euo pipefail
cd "$(dirname "$0")"

declare -A ID=([taskmgr]=TaskManager [files]=Files)
# Ship only finished apps: each one has a launcher in bin/.
APPS=("$@")
if [ ${#APPS[@]} -eq 0 ]; then
    for b in bin/harbor-*; do APPS+=("${b#bin/harbor-}"); done
fi

version_of() {
    local key
    key=$(echo "$1" | tr '[:lower:]' '[:upper:]')_VERSION
    sed -n "s/^$key = \"\(.*\)\"/\1/p" src/harbor/__init__.py
}

control() {   # app version stage
    local app=$1 v=$2 stage=$3 depends recommends desc
    depends="python3 (>= 3.10), python3-gi, gir1.2-gtk-4.0, gir1.2-adw-1 (>= 1.7), gir1.2-glib-2.0"
    if [ "$app" = files ]; then
        depends="$depends, 7zip"
        recommends="7zip-rar, python3-qrcode, gvfs-backends"
        desc="Windows 11-style File Explorer for GNOME
 Harbor Files: a File Explorer with tabs, a breadcrumb address bar,
 recursive search, Home and This PC pages, Quick access pins, drives
 with mount and eject, trash with restore, cut / copy / paste with
 progress, 7-Zip compress and extract, a resizable preview pane (Office
 files through Craft Suite or LibreOffice), Customize folder, network
 locations (FTP, SFTP, SMB, WebDAV) and Share over Wi-Fi with a PIN.
 Delete moves to the trash; permanent delete asks first."
    else
        recommends="pkexec, network-manager, zram-tools"
        desc="Windows 11-style Task Manager for GNOME
 Harbor Task Manager: processes grouped by app with End task and
 Efficiency mode, live CPU / memory / swap / disk / network / GPU graphs,
 a Memory Sweep page (biggest apps, extra helper copies, files held in
 RAM in /tmp and /dev/shm, with suggestions to free memory), memory
 optimization settings for zram, swap file and zswap, a high-memory
 warning, app history, startup apps, users, details with priorities, and
 systemd services. Actions that need root ask through polkit."
    fi
    cat > "$stage/DEBIAN/control" <<CTL
Package: harbor-$app
Version: $v
Section: gnome
Priority: optional
Architecture: all
Depends: $depends
Recommends: $recommends
Breaks: harbor (<< 0.6)
Replaces: harbor (<< 0.6)
Maintainer: Md. Hafijur Rahman <mdhafijur.rahman@ogr.deu.edu.tr>
Installed-Size: $(du -sk "$stage/usr" | cut -f1)
Description: $desc
CTL
}

build() {
    local a=$1 v stage lib n f
    v=$(version_of "$a")
    [ -n "$v" ] || { echo "no ${a^^}_VERSION in src/harbor/__init__.py" >&2; exit 1; }
    stage=$(mktemp -d)       # dpkg rejects the 777 dirs of the NTFS checkout
    chmod 755 "$stage"
    lib="$stage/usr/lib/harbor-$a/harbor"
    for f in src/harbor/common.py src/harbor/"$a".py src/harbor/"$a"_*.py; do
        [ -f "$f" ] && install -Dm644 "$f" "$lib/$(basename "$f")"
    done
    # this package's version becomes VERSION (About dialogs)
    sed "s/^VERSION = .*/VERSION = \"$v\"/" src/harbor/__init__.py > "$lib/__init__.py"
    chmod 644 "$lib/__init__.py"
    install -Dm755 "bin/harbor-$a" "$stage/usr/bin/harbor-$a"
    n=io.github.hafij_deu.Harbor.${ID[$a]}
    install -Dm644 "data/$n.desktop" "$stage/usr/share/applications/$n.desktop"
    install -Dm644 "data/icons/$n.svg" "$stage/usr/share/icons/hicolor/scalable/apps/$n.svg"
    if [ "$a" = taskmgr ]; then
        # Memory Sweep page: root helper for swap / zram / zswap and its polkit action.
        install -Dm755 data/harbor-swap "$stage/usr/libexec/harbor/harbor-swap"
        install -Dm644 data/io.github.hafij_deu.Harbor.policy \
            "$stage/usr/share/polkit-1/actions/io.github.hafij_deu.Harbor.policy"
    else
        install -Dm644 src/harbor/qr.py "$lib/qr.py"
        # Harbor Files' own icon set (harbor-* names), found via the app's icon search path.
        (cd data/icons && find hicolor -name 'harbor-*.svg') | while read -r f; do
            install -Dm644 "data/icons/$f" "$stage/usr/share/harbor/icons/$f"
        done
        # blank Office files for New > Document / Spreadsheet / Presentation
        for f in data/templates/*; do
            install -Dm644 "$f" "$stage/usr/share/harbor/templates/$(basename "$f")"
        done
    fi
    install -Dm644 LICENSE "$stage/usr/share/doc/harbor-$a/copyright"

    mkdir -p "$stage/DEBIAN"
    control "$a" "$v" "$stage"
    cat > "$stage/DEBIAN/postinst" <<'SH'
#!/bin/sh
set -e
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q /usr/share/icons/hicolor || true
command -v update-desktop-database >/dev/null && update-desktop-database -q /usr/share/applications || true
SH
    cp "$stage/DEBIAN/postinst" "$stage/DEBIAN/postrm"
    chmod 755 "$stage/DEBIAN/postinst" "$stage/DEBIAN/postrm"
    mkdir -p dist
    dpkg-deb --root-owner-group --build "$stage" "dist/harbor-${a}_${v}_all.deb"
    rm -rf "$stage"
}

for a in "${APPS[@]}"; do
    [ -n "${ID[$a]:-}" ] || { echo "unknown app: $a" >&2; exit 1; }
    build "$a"
done
