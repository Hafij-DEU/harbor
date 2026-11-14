# Harbor

Windows 11-style apps for GNOME, written in Python with GTK 4 and libadwaita.
They follow the GNOME light/dark mode and accent colour.

## Harbor Task Manager

- **Processes** grouped into Apps, Background processes and System processes,
  with live CPU, memory, disk and network columns, End task, Efficiency mode,
  Run new task and Properties.
- **Performance**: 60-second graphs for CPU (overall or per core), Memory
  (with compressed zram memory), Swap, every disk, every network adapter and
  the GPU.
- **Memory Sweep**: what fills your RAM, suggestions to free it, and the
  files held in `/tmp` and `/dev/shm`.
- **App history**, **Startup apps**, **Users**, **Details** (End process
  tree, Set priority) and **Services** (start, stop, restart).
- **Settings**: update speed, start page, high-memory warning, and Memory
  optimization (zram size and compression, swap file, zswap, swappiness,
  cache pressure). Changes that need root ask for your password through
  polkit.

Linux has no per-process network speed without root, so the Network column
shows open connections per process; the total at the top is real.

## Harbor Files

- **Tabs**, back / forward / up, a breadcrumb address bar you can type a
  path into (Ctrl+L), and search in the current folder (Ctrl+F).
- **Command bar**: New folder or file, cut, copy, paste, rename, delete,
  Sort, View (large icons, medium icons, details, hidden items) and a
  Details pane.
- **Sidebar**: Home, Quick access pins, This PC with every drive (mount and
  eject), Network and Trash (restore, empty).
- **Home** page with Quick access and recent files; **This PC** with
  used/free bars for each drive.
- **Right-click menu**: Open, Open with, Open in new tab, Copy as path, Pin
  to Quick access, Compress to ZIP, Extract all, Open in Terminal and
  Properties (size counted in the background).
- Delete sends to the Trash. Permanent delete and Empty trash ask first.
  Copy and move show progress and ask when a name already exists.
- **Preview pane** (resizable) for images, text, PDF and Office files. Office
  previews use Craft Suite or LibreOffice, whichever is installed.
- **7-Zip**: Add to archive (7z, zip, tar.xz…), Extract here, Extract to
  folder, and Test archive, from the right-click menu.
- **Folder colours** follow your accent colour, or pick one per folder with
  **Customize folder** (colour, icon, emblem, your own picture, Restore
  default).
- **Network locations**: add FTP, SFTP, SMB, WebDAV, NFS or AFP addresses to
  the sidebar. Passwords are asked when you connect and kept by the GNOME
  keyring if you choose, never in Harbor's settings.
- **Share over Wi-Fi**: open chosen folders from a phone or another computer
  in a web browser, or as a WebDAV network drive. A 6-digit PIN protects
  each session; 5 wrong tries lock it for 5 minutes. Folders are read-only
  unless you turn uploads on, and uploads never overwrite a file. Sharing
  never starts by itself and stops after 30 minutes (you can change this).
  Scan the QR code on your phone to open it.

Harbor Files does not make itself your default file manager.

The Settings app is in progress.


## Harbor Files for Android

`android/` holds a phone version of Harbor Files (package `com.hafij.harborfiles`, Android 10+), styled after the Xiaomi file manager.

- **Recent** first: every new file of any kind, grouped by day and by the app or folder it came from.
- **Browse**: categories (images, videos, audio, documents, APKs, archives), Downloads, favourites, Recently deleted and storage volumes.
- **Connect**: start an FTP server on the phone (user `harbor`, a 6-digit PIN, QR code, optional read-only, idle stop), or open a Harbor PC share (WebDAV, the PIN shown in Harbor Files on the desktop) or any FTP server.
- **Send** through MeshDrop when it is installed, or through any other app.
- Own settings: hidden and system folders, screenshots in Recent, Recently deleted keep time, theme and accent colour, server port.

Build and test:

```bash
cd android
gradle --offline testDebugUnitTest assembleRelease
```

Release builds are signed when `HARBOR_KEYSTORE_FILE`, `HARBOR_KEYSTORE_PASSWORD`, `HARBOR_KEY_ALIAS` and `HARBOR_KEY_PASSWORD` are set; otherwise the debug key is used. `HARBOR_DAV="<port> <pin>"` runs an extra test against a live desktop share.

## Install

Each app is its own package. Download `harbor-files_<version>_all.deb`
and/or `harbor-taskmgr_<version>_all.deb` from the
[releases](https://github.com/Hafij-DEU/harbor/releases) (tags
`files-v…` and `taskmgr-v…`) and run:

```bash
sudo apt install ./harbor-files_0.5.1_all.deb ./harbor-taskmgr_0.3.0_all.deb
```

They replace the older combined `harbor` package.

Then open **Harbor Files** or **Harbor Task Manager** from the app menu,
or run `harbor-files [folder]` / `harbor-taskmgr`
(`harbor-taskmgr --page performance:memory` opens a page directly).

## Build and test

```bash
./build-deb.sh            # both apps; ./build-deb.sh files for one
PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/test_files.py tests/test_taskmgr.py tests/test_taskmgr_sweep.py
tools/shot.sh taskmgr shot.png --do "win.show_page('performance')"
```

`tools/shot.sh` renders the window in a private headless GNOME Shell with
`HARBOR_DRY_RUN=1`, so nothing on the real system changes.

## Licence

MIT. Author: Md. Hafijur Rahman.
