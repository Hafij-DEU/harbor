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

Files and Settings apps are in progress.

## Install

Download `harbor_<version>_all.deb` from the
[releases](https://github.com/Hafij-DEU/harbor/releases) and run:

```bash
sudo apt install ./harbor_0.2.0_all.deb
```

Then open **Harbor Task Manager** from the app menu, or run `harbor-taskmgr`
(`harbor-taskmgr --page performance:memory` opens a page directly).

## Build and test

```bash
./build-deb.sh
PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/test_taskmgr.py tests/test_taskmgr_sweep.py
tools/shot.sh taskmgr shot.png --do "win.show_page('performance')"
```

`tools/shot.sh` renders the window in a private headless GNOME Shell with
`HARBOR_DRY_RUN=1`, so nothing on the real system changes.

## Licence

MIT. Author: Md. Hafijur Rahman.
