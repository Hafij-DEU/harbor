"""Harbor Files: the "Share over Wi-Fi" dialog.

The server runs in its own process (files_sharetray) with a "Send by Wi-Fi"
tray icon, so sharing keeps going after Harbor Files quits. It stops with the
Stop button, the tray menu or the auto-stop timer. It never starts by itself.
This dialog talks to it over the session bus.
"""

import json
import os
import subprocess
import sys
import time

from gi.repository import Adw, Gdk, Gio, GLib, Gtk

from . import common
from . import files_core as fc
from . import qr
from .files_share import BUS_NAME, OBJECT_PATH

STOP_LABELS = {15: "15 minutes", 30: "30 minutes", 60: "1 hour", 120: "2 hours", 0: "Never"}


def share_folders(settings):
    return [dict(f) for f in settings.data["share_folders"]]


def add_share_folder(settings, path, upload=False):
    path = os.path.normpath(path)
    if any(f["path"] == path for f in settings.data["share_folders"]):
        return False
    settings.data["share_folders"].append({"path": path, "upload": bool(upload)})
    settings.save()
    return True


def set_share_upload(settings, path, upload):
    for f in settings.data["share_folders"]:
        if f["path"] == path:
            f["upload"] = bool(upload)
    settings.save()


def remove_share_folder(settings, path):
    settings.data["share_folders"] = [f for f in settings.data["share_folders"]
                                      if f["path"] != path]
    settings.save()


def qr_texture(text, scale=6, quiet=4):
    """QR code as a Gdk texture (no cairo bindings needed)."""
    m = qr.best_matrix(text)
    n = len(m) + 2 * quiet
    side = n * scale
    rows = []
    for y in range(n):
        line = bytearray()
        for x in range(n):
            my, mx = y - quiet, x - quiet
            dark = 0 <= my < len(m) and 0 <= mx < len(m) and m[my][mx]
            line += (b"\x00\x00\x00" if dark else b"\xff\xff\xff") * scale
        rows.append(bytes(line) * scale)
    data = GLib.Bytes.new(b"".join(rows))
    return Gdk.MemoryTexture.new(side, side, Gdk.MemoryFormat.R8G8B8, data, side * 3), side


def qr_picture(text):
    tex, side = qr_texture(text)
    pic = Gtk.Picture(paintable=tex, can_shrink=False, halign=Gtk.Align.CENTER)
    pic.set_size_request(side, side)
    return pic


class RemoteShare:
    """The running share, as reported by the share process."""

    running = True

    def __init__(self, conn, st):
        self.conn = conn
        self.url = st["url"]
        self.dav_url = st["dav_url"]
        self.qr_text = st["qr_text"]
        self.pin = st["pin"]
        self.host = st["host"]
        self.port = st["port"]
        self.stops_at = st.get("stops_at")
        self.folders = st.get("folders")

    def stop(self):
        try:
            self.conn.call_sync(BUS_NAME, OBJECT_PATH, BUS_NAME, "Stop", None, None,
                                Gio.DBusCallFlags.NO_AUTO_START, 2000, None)
        except GLib.Error:
            pass


def running_share():
    """RemoteShare when a share process is running, else None."""
    try:
        conn = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        r = conn.call_sync(BUS_NAME, OBJECT_PATH, BUS_NAME, "Status", None,
                           GLib.VariantType("(s)"), Gio.DBusCallFlags.NO_AUTO_START,
                           2000, None)
        return RemoteShare(conn, json.loads(r.unpack()[0]))
    except (GLib.Error, ValueError, KeyError):
        return None


def start_share(folders, port, upload_max_mb, minutes):
    """Start the share process. Returns None, or an error message."""
    pkg = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    env = dict(os.environ)
    env["PYTHONPATH"] = pkg + (os.pathsep + env["PYTHONPATH"] if env.get("PYTHONPATH") else "")
    try:
        p = subprocess.Popen([sys.executable, "-m", "harbor.files_sharetray"], env=env,
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                             stderr=subprocess.DEVNULL, start_new_session=True, text=True)
        p.stdin.write(json.dumps({"folders": folders, "port": port,
                                  "upload_max_mb": upload_max_mb, "minutes": minutes}) + "\n")
        p.stdin.close()
        line = p.stdout.readline()
        p.stdout.close()
    except OSError as e:
        return str(e)
    try:
        ans = json.loads(line)
    except ValueError:
        return "the share process did not start"
    return None if ans.get("ok") else ans.get("error", "unknown error")


class ShareDialog(Adw.Dialog):
    def __init__(self, win, add_paths=()):
        super().__init__(title="Share over Wi-Fi", content_width=520, content_height=680)
        self.win = win
        self.app = win.app
        self.settings = win.app.settings
        # add_paths (right-click > Share over Wi-Fi) is a one-off share of just those
        # items: it is not added to the saved list that the sidebar entry reuses
        self.temp = [{"path": os.path.normpath(p), "upload": False} for p in add_paths]
        tv = Adw.ToolbarView()
        tv.add_top_bar(Adw.HeaderBar())
        self.page = Adw.PreferencesPage()
        tv.set_content(self.page)
        self.set_child(tv)
        self._tick = 0
        self._restart = False
        self.server = running_share()
        self._watch = Gio.bus_watch_name(Gio.BusType.SESSION, BUS_NAME,
                                         Gio.BusNameWatcherFlags.NONE, None, self._vanished)
        self.connect("closed", lambda *_: (self._stop_tick(),
                                           Gio.bus_unwatch_name(self._watch)))
        self.build()
        if add_paths:
            # Right-click > Share over Wi-Fi: start now (or restart to include them).
            if self.server is None:
                self.start()
            else:
                self._restart = True
                self.server.stop()

    def _vanished(self, *_):
        if self.server is not None:
            self.server = None
            if self._restart:
                self._restart = False
                self.start()
                return
            self.win.notify_toast("Sharing over Wi-Fi stopped")
            self.build()

    def _stop_tick(self):
        if self._tick:
            GLib.source_remove(self._tick)
            self._tick = 0

    # ------------------------------------------------------------ layout
    def build(self):
        for g in list(getattr(self, "_groups", [])):
            self.page.remove(g)
        self._groups = []
        self._stop_tick()
        running = self.server is not None and self.server.running
        if running:
            for g in self._status_group():
                self._add(g)
        self._add(self._folders_group(running))
        self._add(self._phone_group())
        self._add(self._options_group(running))
        bar = Adw.PreferencesGroup()
        btn = Gtk.Button(label="Stop sharing" if running else "Start sharing",
                         halign=Gtk.Align.CENTER, css_classes=["pill", "destructive-action"
                                                               if running else "suggested-action"])
        btn.set_sensitive(running or bool(self._to_share()))
        btn.connect("clicked", lambda *_: self.stop() if running else self.start())
        bar.add(btn)
        self._add(bar)
        self.set_focus(btn)          # not the selectable URL label

    def _add(self, g):
        self.page.add(g)
        self._groups.append(g)

    def _status_group(self):
        s = self.server
        g = Adw.PreferencesGroup(title="Open on your phone or another computer")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.append(Gtk.Label(label="Scan with your phone: the camera app opens it in the "
                                   "browser, Harbor Files for Android connects with no typing",
                             wrap=True, justify=Gtk.Justification.CENTER,
                             css_classes=["dim-label"]))
        box.append(qr_picture(s.qr_text))
        url = Gtk.Label(label=s.url, selectable=True, css_classes=["title-3"])
        box.append(url)
        pin = Gtk.Label(label="PIN  " + " ".join(s.pin), selectable=True,
                        css_classes=["title-1"])
        box.append(pin)
        self.left = Gtk.Label(css_classes=["dim-label"])
        box.append(self.left)
        g.add(box)
        rows = Adw.PreferencesGroup()
        dav = Adw.ActionRow(title="As a network drive (WebDAV)",
                            subtitle="%s  ·  user “harbor”, password = PIN" % s.dav_url,
                            subtitle_selectable=True)
        rows.add(dav)
        hint = Adw.ActionRow(
            title="Can't connect?",
            subtitle="Both devices must be on the same Wi-Fi. If a firewall is on, allow "
                     "port %d for your network, e.g. sudo ufw allow from %s/24 to any "
                     "port %d proto tcp" % (s.port, s.host.rsplit(".", 1)[0] + ".0", s.port),
            subtitle_selectable=True)
        rows.add(hint)
        self._update_left()
        self._tick = GLib.timeout_add_seconds(20, self._update_left)
        return g, rows

    def _update_left(self):
        s = self.server
        if s is None or not s.running:
            self._tick = 0
            return False
        if s.stops_at:
            mins = max(0, int((s.stops_at - time.time()) // 60))
            self.left.set_label("Stops by itself in %d min" % mins if mins else
                                "Stops in less than a minute")
        else:
            self.left.set_label("Runs until you stop it")
        return True

    def _phone_group(self):
        g = Adw.PreferencesGroup()
        row = Adw.ActionRow(title="Open your phone's files",
                            subtitle="Scan the QR code Harbor Files for Android shows "
                                     "under Connect")
        row.add_prefix(Gtk.Image(icon_name="phone-symbolic"))
        btn = Gtk.Button(label="Scan phone QR…", valign=Gtk.Align.CENTER)
        btn.connect("clicked", lambda *_: (self.close(), self.win.scan_phone()))
        row.add_suffix(btn)
        g.add(row)
        return g

    def _to_share(self):
        folders = self.temp or share_folders(self.settings)
        return [f for f in folders if os.path.isdir(f["path"]) or os.path.isfile(f["path"])]

    def _folders_group(self, running):
        shared = getattr(self.server, "folders", None) if running else None
        if shared is not None or self.temp:
            return self._oneoff_group(shared if shared is not None else self.temp, running)
        g = Adw.PreferencesGroup(title="Shared folders and files",
                                 description="Only these (and what is inside the folders) can "
                                             "be opened. Hidden files are never shared. "
                                             "Right-click a file or folder to share just that, "
                                             "without adding it here.")
        btns = Gtk.Box(spacing=4)
        add = Gtk.Button(label="Add folder…", css_classes=["flat"], sensitive=not running)
        add.connect("clicked", self._pick_folder)
        addf = Gtk.Button(label="Add files…", css_classes=["flat"], sensitive=not running)
        addf.connect("clicked", self._pick_files)
        btns.append(add)
        btns.append(addf)
        g.set_header_suffix(btns)
        folders = share_folders(self.settings)
        if not folders:
            g.add(Adw.ActionRow(title="Nothing shared yet", subtitle="Add a folder or files to share them"))
        for f in folders:
            is_dir = not os.path.isfile(f["path"])
            row = Adw.ActionRow(title=os.path.basename(f["path"]) or f["path"],
                                subtitle=f["path"], sensitive=not running)
            row.add_prefix(Gtk.Image(icon_name="folder-symbolic" if is_dir
                                     else "text-x-generic-symbolic"))
            if is_dir:
                up = Gtk.Switch(active=f["upload"], valign=Gtk.Align.CENTER,
                                tooltip_text="Let other devices upload into this folder")
                up.connect("notify::active",
                           lambda sw, _p, p=f["path"]: set_share_upload(self.settings, p,
                                                                         sw.get_active()))
                row.add_suffix(Gtk.Label(label="Uploads", css_classes=["dim-label"]))
                row.add_suffix(up)
            rm = Gtk.Button(icon_name="list-remove-symbolic", valign=Gtk.Align.CENTER,
                            css_classes=["flat"], tooltip_text="Stop sharing this folder")
            rm.connect("clicked", lambda _b, p=f["path"]: (
                remove_share_folder(self.settings, p), self.build()))
            row.add_suffix(rm)
            g.add(row)
        return g

    def _oneoff_group(self, folders, running):
        if running:
            g = Adw.PreferencesGroup(title="Sharing now")
        else:
            g = Adw.PreferencesGroup(title="Share only this",
                                     description="A one-time share: it is not added to your "
                                                 "saved list. Use the Share button to share "
                                                 "your saved list.")
        for f in folders:
            is_dir = not os.path.isfile(f["path"])
            row = Adw.ActionRow(title=os.path.basename(f["path"]) or f["path"],
                                subtitle=f["path"])
            row.add_prefix(Gtk.Image(icon_name="folder-symbolic" if is_dir
                                     else "text-x-generic-symbolic"))
            if is_dir and not running:
                up = Gtk.Switch(active=f["upload"], valign=Gtk.Align.CENTER,
                                tooltip_text="Let other devices upload into this folder")
                up.connect("notify::active", lambda sw, _p, f=f: f.update(upload=sw.get_active()))
                row.add_suffix(Gtk.Label(label="Uploads", css_classes=["dim-label"]))
                row.add_suffix(up)
            elif is_dir and f.get("upload"):
                row.add_suffix(Gtk.Label(label="Uploads on", css_classes=["dim-label"]))
            g.add(row)
        return g

    def _options_group(self, running):
        g = Adw.PreferencesGroup()
        keys = list(fc.SHARE_AUTOSTOP)
        combo = Adw.ComboRow(title="Stop sharing after",
                             model=Gtk.StringList.new([STOP_LABELS[k] for k in keys]),
                             sensitive=not running)
        cur = self.settings.get("share_autostop")
        combo.set_selected(keys.index(cur) if cur in keys else 1)
        combo.connect("notify::selected",
                      lambda c, _p: self.settings.set("share_autostop",
                                                      keys[c.get_selected()]))
        g.add(combo)
        return g

    def _pick_folder(self, *_):
        fd = Gtk.FileDialog(title="Choose a folder to share")

        def done(d, res):
            try:
                g = d.select_folder_finish(res)
            except GLib.Error:
                return
            if g is not None and g.get_path():
                add_share_folder(self.settings, g.get_path())
                self.build()
        fd.select_folder(self.win, None, done)

    def _pick_files(self, *_):
        fd = Gtk.FileDialog(title="Choose files to share")

        def done(d, res):
            try:
                files = d.open_multiple_finish(res)
            except GLib.Error:
                return
            for i in range(files.get_n_items() if files else 0):
                path = files.get_item(i).get_path()
                if path:
                    add_share_folder(self.settings, path)
            self.build()
        fd.open_multiple(self.win, None, done)

    # ------------------------------------------------------------ server
    def start(self):
        folders = self._to_share()
        if not folders:
            return
        s = self.settings
        if common.DRY_RUN:
            common.DRY_RUN_LOG.append(["share-start", [f["path"] for f in folders]])
            return
        err = start_share(folders, s.get("share_port"), s.get("share_upload_max_mb"),
                          s.get("share_autostop"))
        if err:
            self.win.notify_toast("Can't start sharing: %s" % err, timeout=6)
            return
        self.server = running_share()
        self.build()

    def stop(self):
        if self.server is not None:
            self.server.stop()        # the name vanishes, then _vanished() rebuilds
