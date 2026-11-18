<div align="center">

<img src="../data/icons/io.github.hafij_deu.Harbor.Files.svg" width="88" alt="Harbor Files icon">

# Harbor Files

**A Windows 11-style file manager for GNOME.**

[![Release](https://img.shields.io/github/v/release/Hafij-DEU/harbor?filter=files-v*&label=release&color=e8590c)](https://github.com/Hafij-DEU/harbor/releases?q=files-v&expanded=true)
![Python](https://img.shields.io/badge/Python-GTK%204-3776ab?logo=python&logoColor=white)

[← Harbor](../README.md) · [Task Manager](taskmgr.md) · [Settings](../settings/README.md) · [Android](../android/README.md)

</div>

---

![Harbor Files home](files-home-light.png)

## Contents

- [Browse](#browse) · [File-type icons](#file-type-icons) · [Look and layout](#look-and-layout)
- [Files and folders](#files-and-folders) · [Preview and properties](#preview-and-properties) · [Archives](#archives)
- [Network places](#network-places) · [Share over Wi-Fi](#share-over-wi-fi) · [Scan phone QR](#scan-phone-qr)
- [Gallery](#gallery) · [Install](#install)

## Browse

- **Tabs**, back / forward / up and a breadcrumb address bar. Type a path with <kbd>Ctrl</kbd>+<kbd>L</kbd>; search the current folder with <kbd>Ctrl</kbd>+<kbd>F</kbd>.
- **Sidebar**: Home, Quick access pins, This PC with every drive (mount and eject), Network and Trash.
- **Home** shows Quick access and recent files.
- **This PC** shows a used/free bar for each drive. Free space updates live and the cards redraw when a drive is mounted or removed.

## File-type icons

Every common extension has its own icon: the file type's picture plus a coloured badge with the extension name. 133 icons cover packages (DEB, RPM, APK, AppImage, EXE, MSI), archives, documents, spreadsheets, slides, images, audio, video, subtitles, fonts and source code.

![File-type icons](files-type-icons.png)

- The extension decides the icon, so every `.deb` looks the same in every view.
- An unknown extension keeps the icon of its category (text, image, audio…).
- Image thumbnails still show instead of an icon.
- **Options › Icons › System** switches to your icon theme instead.

The table lives in `EXT_TABLE` in [`src/harbor/files_icons.py`](../src/harbor/files_icons.py); `python3 tools/make_icons.py` redraws the SVGs.

## Look and layout

- **Windows 11 command bar** or a **classic ribbon**, chosen in the welcome tour or in Options.
- Large icons, medium icons or details view, with hidden items on demand.
- **Folder colours** follow your accent colour. **Customize folder** sets a colour, icon, emblem or your own picture per folder.
- A first-run **welcome tour** sets the design, colours and layout.
- **Glass effect** (Options): Off, Soft, Strong or the same as all Harbor apps. Every glass option (style, opacity, rim, glare, Glow) is in [Harbor Settings › Personalisation](../settings/README.md#glass-effects).

| Command bar | Ribbon |
| --- | --- |
| ![Windows 11 menu](files-win11-menu.png) | ![Ribbon](files-ribbon.png) |

## Files and folders

- New folder or file, cut, copy, paste, rename and delete from the command bar.
- **Right-click menu**: Open, Open with, Open in new tab, Copy as path, Pin to Quick access, Compress to ZIP, Extract all, Open in Terminal and Properties.
- Delete sends files to the Trash. Permanent delete and Empty trash ask first.
- Copy and move show progress and ask when a name already exists.

## Preview and properties

- A resizable **Preview pane** shows images, text, PDF and Office files. Office previews use Craft Suite or LibreOffice, whichever is installed.
- A **Details pane** shows the selected item's information.
- **Properties** counts folder sizes in the background.

| Preview | Properties |
| --- | --- |
| ![Image preview](files-preview-image.png) | ![Properties](files-properties.png) |

## Archives

With 7-Zip installed, the right-click menu adds **Add to archive** (7z, zip, tar.xz…), **Extract here**, **Extract to folder** and **Test archive**.

## Network places

Add FTP, SFTP, SMB, WebDAV, NFS or AFP addresses to the sidebar. Harbor asks for the password when you connect. The GNOME keyring keeps it if you choose; Harbor's settings never store it.

## Share over Wi-Fi

Open chosen folders from a phone or another computer, in a web browser or as a WebDAV network drive.

![Share over Wi-Fi](files-share-wifi.png)

- A 6-digit PIN protects each session. Five wrong tries lock it for 5 minutes.
- Folders are read-only unless you turn uploads on. Uploads never overwrite a file.
- Sharing never starts by itself and stops after 30 minutes (you can change this).
- The QR code carries the PIN: the phone camera opens the page and signs in. [Harbor Files for Android](../android/README.md) connects with no typing.
- Sharing keeps going after you quit Harbor Files. A **Send by Wi-Fi** tray icon shows the QR code and PIN, copies the address and stops the share.

## Scan phone QR

Open Harbor Files for Android's storage in a new tab over FTP. The PIN is never saved.

- **Scan with this PC**: hold the phone's QR code up to the webcam. Virtual cameras such as v4l2loopback are skipped.
- **Scan with the phone**: the PC shows a one-time code for 5 minutes. Scan it in Harbor Files › Connect; the phone shares itself and sends its address and PIN back. Needs the Android app 0.1.2 or later.

## Gallery

<table>
<tr>
<td width="33%"><img src="files-home-dark.png" alt=""><br><sub>Home, dark</sub></td>
<td width="33%"><img src="files-thispc.png" alt=""><br><sub>This PC</sub></td>
<td width="33%"><img src="files-context-menu.png" alt=""><br><sub>Right-click menu</sub></td>
</tr>
<tr>
<td><img src="files-classic-menu.png" alt=""><br><sub>Classic menu</sub></td>
<td><img src="files-ribbon-view.png" alt=""><br><sub>Ribbon › View</sub></td>
<td><img src="files-details-pane.png" alt=""><br><sub>Details pane</sub></td>
</tr>
<tr>
<td><img src="files-folder-details.png" alt=""><br><sub>Details view</sub></td>
<td><img src="files-folder-icons.png" alt=""><br><sub>Folder colours</sub></td>
<td><img src="files-icons-large.png" alt=""><br><sub>Large icons</sub></td>
</tr>
<tr>
<td><img src="files-preview-text.png" alt=""><br><sub>Text preview</sub></td>
<td><img src="files-trash.png" alt=""><br><sub>Trash</sub></td>
<td><img src="files-options-general.png" alt=""><br><sub>Options › General</sub></td>
</tr>
<tr>
<td><img src="files-options-view.png" alt=""><br><sub>Options › View</sub></td>
<td><img src="files-system-theme-light.png" alt=""><br><sub>System icon theme, light</sub></td>
<td><img src="files-system-theme-dark.png" alt=""><br><sub>System icon theme, dark</sub></td>
</tr>
</table>

## Install

```bash
sudo apt install ./harbor-files_*_all.deb
harbor-files [folder]
```

Get the `.deb` from [Releases](https://github.com/Hafij-DEU/harbor/releases?q=files-v&expanded=true). Harbor Files does not make itself your default file manager.

---

<div align="center"><sub><a href="../README.md">Harbor</a> · <a href="taskmgr.md">Task Manager</a> · <a href="../settings/README.md">Settings</a> · <a href="../android/README.md">Android</a></sub></div>
