"""Harbor Files: the Share over Wi-Fi runner.

Harbor Files starts this as its own process (python3 -m harbor.files_sharetray)
so sharing keeps going after Harbor Files quits. It reads one JSON line of
settings on stdin, starts the server, answers one JSON line on stdout, then
shows a "Send by Wi-Fi" tray icon until the share stops (Stop, the auto-stop
timer, or the tray menu). The process exits when the share stops.

Session D-Bus name io.github.hafij_deu.Harbor.Share (no service file, so it
is never started by D-Bus): Status() -> JSON, Stop().
The PIN is never logged; it only leaves this process over the user's own
session bus and in the QR window.
"""

import json
import os
import shutil
import signal
import subprocess
import sys
import time

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import Gdk, GdkPixbuf, Gio, GLib, Gtk  # noqa: E402

try:
    gi.require_version("AyatanaAppIndicator3", "0.1")
    from gi.repository import AyatanaAppIndicator3 as AppIndicator  # noqa: E402
except (ValueError, ImportError):
    AppIndicator = None

from . import qr  # noqa: E402
from .files_share import BUS_NAME, OBJECT_PATH, ShareServer  # noqa: E402
XML = """
<node>
  <interface name="io.github.hafij_deu.Harbor.Share">
    <method name="Status"><arg type="s" name="json" direction="out"/></method>
    <method name="Stop"/>
    <method name="ShowQr"/>
  </interface>
</node>
"""


def qr_pixbuf(text, scale=6, quiet=4):
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
    return GdkPixbuf.Pixbuf.new_from_bytes(GLib.Bytes.new(b"".join(rows)),
                                           GdkPixbuf.Colorspace.RGB, False, 8,
                                           side, side, side * 3)


def time_left(stops_at):
    if not stops_at:
        return "Runs until you stop it"
    mins = max(0, int((stops_at - time.time()) // 60))
    return "Stops by itself in %d min" % mins if mins else "Stops in less than a minute"


def own_name(conn):
    """True when this process got the name (no other share is running)."""
    r = conn.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus",
                       "org.freedesktop.DBus", "RequestName",
                       GLib.Variant("(su)", (BUS_NAME, 4)),     # DO_NOT_QUEUE
                       GLib.VariantType("(u)"), Gio.DBusCallFlags.NONE, -1, None)
    return r.unpack()[0] == 1


class Runner:
    def __init__(self, server, conn):
        self.server = server
        self.conn = conn
        self.qr_win = None
        node = Gio.DBusNodeInfo.new_for_xml(XML)
        conn.register_object(OBJECT_PATH, node.interfaces[0], self._call)
        self.indicator = None
        if AppIndicator is not None:
            ind = AppIndicator.Indicator.new(
                "harbor-share", "network-wireless-symbolic",
                AppIndicator.IndicatorCategory.APPLICATION_STATUS)
            ind.set_title("Send by Wi-Fi")
            ind.set_menu(self._menu())
            ind.set_status(AppIndicator.IndicatorStatus.ACTIVE)
            self.indicator = ind

    def _call(self, conn, sender, path, iface, method, params, inv):
        if method == "Status":
            inv.return_value(GLib.Variant("(s)", (json.dumps(self.server.status()),)))
        elif method == "Stop":
            inv.return_value(None)
            GLib.idle_add(self.stop)
        elif method == "ShowQr":
            inv.return_value(None)
            GLib.idle_add(lambda: self.show_qr() or False)

    def _menu(self):
        menu = Gtk.Menu()
        head = Gtk.MenuItem(label="Send by Wi-Fi: " + self.server.url, sensitive=False)
        menu.append(head)
        menu.append(Gtk.SeparatorMenuItem())
        for label, cb in (("Show QR & PIN", self.show_qr),
                          ("Copy address", self.copy_address),
                          ("Open Harbor Files", self.open_files),
                          ("Stop sharing", self.stop)):
            it = Gtk.MenuItem(label=label)
            it.connect("activate", lambda _i, cb=cb: cb())
            menu.append(it)
        menu.show_all()
        return menu

    def show_qr(self):
        if self.qr_win is not None:
            self.qr_win.present()
            return
        s = self.server
        w = Gtk.Window(title="Send by Wi-Fi", resizable=False)
        w.set_icon_name("network-wireless-symbolic")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, margin=18)
        box.add(Gtk.Label(label="Scan with your phone's camera or Harbor Files"))
        box.add(Gtk.Image.new_from_pixbuf(qr_pixbuf(s.qr_text)))
        url = Gtk.Label(selectable=True)
        url.set_markup("<big>%s</big>" % GLib.markup_escape_text(s.url))
        box.add(url)
        pin = Gtk.Label(selectable=True)
        pin.set_markup("<span size='xx-large' weight='bold'>PIN  %s</span>" % " ".join(s.pin))
        box.add(pin)
        left = Gtk.Label(label=time_left(s.stops_at))
        left.get_style_context().add_class("dim-label")
        box.add(left)
        stop = Gtk.Button(label="Stop sharing")
        stop.get_style_context().add_class("destructive-action")
        stop.connect("clicked", lambda *_: self.stop())
        box.add(stop)
        w.add(box)

        def tick():
            if self.qr_win is None:
                return False
            left.set_label(time_left(s.stops_at))
            return True
        GLib.timeout_add_seconds(20, tick)

        def closed(*_):
            self.qr_win = None
        w.connect("destroy", closed)
        self.qr_win = w
        w.show_all()

    def copy_address(self):
        cb = Gtk.Clipboard.get_default(Gdk.Display.get_default())
        cb.set_text(self.server.url, -1)      # address only, never the PIN
        cb.store()

    def open_files(self):
        exe = shutil.which("harbor-files")
        cmd = [exe] if exe else [sys.executable, "-c",
                                  "import sys; from harbor.files import main; sys.exit(main(sys.argv))"]
        subprocess.Popen(cmd, start_new_session=True, stdin=subprocess.DEVNULL,
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def stop(self):
        self.server.stop()          # on_stop quits the main loop
        return False


def saved_config(settings=None):
    """Share config from Harbor Files' saved settings (what was shared last time)."""
    if settings is None:
        from .files_core import Settings
        settings = Settings()
    d = settings.data
    return {"folders": [dict(f) for f in d.get("share_folders", [])],
            "port": d.get("share_port"), "upload_max_mb": d.get("share_upload_max_mb"),
            "minutes": d.get("share_autostop")}


def shareable(folders):
    return [f for f in folders
            if os.path.isdir(f.get("path", "")) or os.path.isfile(f.get("path", ""))]


def notice(text, secondary):
    d = Gtk.MessageDialog(message_type=Gtk.MessageType.INFO, buttons=Gtk.ButtonsType.OK,
                          text=text, title="Send by Wi-Fi")
    d.format_secondary_text(secondary)
    d.run()
    d.destroy()


def main(argv=None):
    """Read the config on stdin (from Harbor Files), or with --again use the
    saved share list and show the QR code at once."""
    again = "--again" in (sys.argv[1:] if argv is None else argv)
    if again:
        cfg = saved_config()
    else:
        try:
            cfg = json.loads(sys.stdin.readline() or "{}")
        except ValueError:
            cfg = {}

    def answer(obj):
        if again:
            if obj.get("error"):
                notice("Can't share over Wi-Fi", obj["error"])
            return
        sys.stdout.write(json.dumps(obj) + "\n")
        sys.stdout.flush()
        null = os.open(os.devnull, os.O_WRONLY)   # Harbor Files stops reading here
        os.dup2(null, 1)
        os.close(null)

    folders = shareable(cfg.get("folders", []))
    if not folders:
        answer({"error": "Nothing to share yet. In Harbor Files, right-click a file or "
                         "folder and choose Share over Wi-Fi." if again
                else "No folders to share"})
        return 1
    try:
        conn = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        if not own_name(conn):
            if again:      # already sharing: just show its QR code
                conn.call_sync(BUS_NAME, OBJECT_PATH, BUS_NAME, "ShowQr", None, None,
                               Gio.DBusCallFlags.NO_AUTO_START, 5000, None)
                return 0
            answer({"error": "Sharing is already on"})
            return 1
    except GLib.Error as e:
        answer({"error": e.message})
        return 1
    server = ShareServer(folders, port=int(cfg.get("port") or 8765),
                         upload_max_mb=int(cfg.get("upload_max_mb") or 4096),
                         minutes=int(cfg.get("minutes") or 0),
                         on_stop=lambda: GLib.idle_add(Gtk.main_quit))
    try:
        server.start()
    except OSError as e:
        answer({"error": str(e)})
        return 1
    runner = Runner(server, conn)
    answer({"ok": True})
    if again:
        runner.show_qr()
    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, sig, lambda *_: server.stop() or False)
    Gtk.main()
    server.stop()
    return 0


if __name__ == "__main__":
    sys.exit(main())
