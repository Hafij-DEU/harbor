<div align="center">

<img src="design/app-icon-512.png" width="112" alt="Harbor icon">

# Harbor

**Windows 11-style Files, Task Manager and Settings for GNOME, plus a file manager for Android.**

They follow your GNOME light/dark mode and accent colour, and they edit the real system.

[![Files](https://img.shields.io/github/v/release/Hafij-DEU/harbor?filter=files-v*&label=&logo=github&color=e8590c)](https://github.com/Hafij-DEU/harbor/releases?q=files-v&expanded=true)
[![Task Manager](https://img.shields.io/github/v/release/Hafij-DEU/harbor?filter=taskmgr-v*&label=&logo=github&color=1e88e5)](https://github.com/Hafij-DEU/harbor/releases?q=taskmgr-v&expanded=true)
[![Settings](https://img.shields.io/github/v/release/Hafij-DEU/harbor?filter=settings-v*&label=&logo=github&color=5e35b1)](https://github.com/Hafij-DEU/harbor/releases?q=settings-v&expanded=true)
[![Android](https://img.shields.io/github/v/release/Hafij-DEU/harbor?filter=android-v*&label=&logo=github&color=3ddc84)](https://github.com/Hafij-DEU/harbor/releases?q=android-v&expanded=true)
<br>
[![Licence: MIT](https://img.shields.io/badge/licence-MIT-informational)](LICENSE)
![GNOME](https://img.shields.io/badge/GNOME-GTK%204%20%C2%B7%20libadwaita-4a86cf?logo=gnome&logoColor=white)
![Ubuntu](https://img.shields.io/badge/Ubuntu-.deb-e95420?logo=ubuntu&logoColor=white)

[Apps](#apps) · [Screenshots](#screenshots) · [Install](#install) · [Build](#build-and-test) · [Docs](#documentation)

</div>

---

## Apps

| | App | What it is | Stack | Guide |
| :-: | --- | --- | --- | --- |
| <img src="data/icons/io.github.hafij_deu.Harbor.Files.svg" width="40"> | **Harbor Files** | File manager with tabs, a ribbon or command bar, previews, folder colours, an icon for every file type, 7-Zip, network places and Share over Wi-Fi. | Python · GTK 4 | [docs/files.md](docs/files.md) |
| <img src="data/icons/io.github.hafij_deu.Harbor.TaskManager.svg" width="40"> | **Harbor Task Manager** | Processes, live performance graphs, Memory Sweep, startup apps, services and memory tuning (zram, swap, zswap). | Python · GTK 4 | [docs/taskmgr.md](docs/taskmgr.md) |
| <img src="settings/assets/win11-settings.svg" width="40"> | **Harbor Settings** | Settings app that edits the real system: display, sound, network, Bluetooth, apps, updates, desktop and GNOME extensions. | Rust · egui | [settings/README.md](settings/README.md) |
| <img src="android/app/src/main/res/drawable-nodpi/ic_launcher_art.png" width="40"> | **Harbor Files for Android** | Phone file manager with a Recent feed, categories, an FTP server and a one-scan link to the PC. | Java · Android 10+ | [android/README.md](android/README.md) |

Each app is a separate package with its own version. Release tags are `files-v…`, `taskmgr-v…`, `settings-v…` and `android-v…`.

## Screenshots

<table>
<tr>
<td width="50%"><a href="docs/files.md"><img src="docs/files-home-light.png" alt="Harbor Files home page"></a><br><sub><b>Files</b>: Home with Quick access and recent files</sub></td>
<td width="50%"><a href="docs/files.md#file-type-icons"><img src="docs/files-type-icons.png" alt="Harbor Files file-type icons"></a><br><sub><b>Files</b>: an icon for every file extension</sub></td>
</tr>
<tr>
<td><a href="docs/taskmgr.md"><img src="docs/taskmgr-processes.png" alt="Task Manager processes"></a><br><sub><b>Task Manager</b>: Processes grouped by Apps, Background and System</sub></td>
<td><a href="settings/README.md"><img src="settings/assets/screenshot.png" alt="Harbor Settings"></a><br><sub><b>Settings</b>: System page</sub></td>
</tr>
<tr>
<td><a href="docs/taskmgr.md#performance"><img src="docs/taskmgr-performance-cpu.png" alt="Task Manager CPU graph"></a><br><sub><b>Task Manager</b>: Performance › CPU</sub></td>
<td><a href="docs/files.md#share-over-wi-fi"><img src="docs/files-share-wifi.png" alt="Share over Wi-Fi"></a><br><sub><b>Files</b>: Share over Wi-Fi with a QR code and PIN</sub></td>
</tr>
</table>

More pictures are in the [Files guide](docs/files.md#gallery) and the [Task Manager guide](docs/taskmgr.md#gallery).

## Install

Download the packages you want from [Releases](https://github.com/Hafij-DEU/harbor/releases) and install them with apt:

```bash
sudo apt install ./harbor-files_*_all.deb ./harbor-taskmgr_*_all.deb ./win11-settings_*_amd64.deb
```

| Package | Command | Notes |
| --- | --- | --- |
| `harbor-files` | `harbor-files [folder]` | Does not make itself your default file manager. |
| `harbor-taskmgr` | `harbor-taskmgr [--page performance:memory]` | Root-only changes ask through polkit. |
| `win11-settings` | `win11-settings` | Installs under `/usr/local`; Ubuntu's Settings stays. |
| `HarborFiles-<v>.apk` | Android app | Install on the phone; Android 10 or later. |

The packages replace the old combined `harbor` package.

## Build and test

```bash
./build-deb.sh                 # Files and Task Manager; ./build-deb.sh files for one
PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/
tools/shot.sh files shot.png   # screenshot in a private headless GNOME Shell
```

- **Settings**: `cd settings && cargo test --release && ./packaging/build-deb.sh`
- **Android**: `cd android && gradle --offline testDebugUnitTest assembleRelease`

`tools/shot.sh` runs the app with `HARBOR_DRY_RUN=1` and private XDG folders, so nothing on the real system changes.

## Documentation

| Guide | Covers |
| --- | --- |
| [docs/files.md](docs/files.md) | Every Harbor Files feature, the file-type icons, sharing and phone links |
| [docs/taskmgr.md](docs/taskmgr.md) | Task Manager pages, Memory Sweep and memory optimisation |
| [settings/README.md](settings/README.md) | Settings pages, App Hub integration, cleaner, environment variables |
| [android/README.md](android/README.md) | Android app features, build and signing |

## Repository layout

```text
src/harbor/      Files and Task Manager (Python, GTK 4, libadwaita)
data/            desktop files, polkit policy, bundled harbor-* icon set
settings/        Harbor Settings (Rust, eframe/egui)
android/         Harbor Files for Android (Gradle)
tools/           icon generators and the headless screenshot harness
tests/           pytest suite
docs/            guides and screenshots
design/          app icon sources
```

## Licence

[MIT](LICENSE) © Md. Hafijur Rahman
