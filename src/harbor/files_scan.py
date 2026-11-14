"""Harbor Files: "Scan phone QR" — open Harbor for Android's files over FTP.

Two ways, one per page of the dialog:

* Scan with this PC: the webcam reads the phone's ftp://harbor:PIN@ip:port/
  code. Virtual cameras (v4l2loopback, e.g. MeshDrop's) and metadata nodes
  are skipped.
* Scan with the phone: the PC shows harborpair://ip:port/TOKEN. The phone
  starts its share and POSTs its ftp:// code with the token to /pair on a
  one-shot listener; the PC checks the token and that the share is on the
  phone that sent it, then opens it.

The PIN and the token go straight into a Gio.MountOperation (gvfs is told
PasswordSave.NEVER) or a compare; they are never logged or saved.
"""

import glob
import hmac
import json
import os
import secrets
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import unquote, urlsplit

from gi.repository import Adw, Gio, GLib, Gtk

try:
    import gi
    gi.require_version("Gst", "1.0")
    from gi.repository import Gst
except (ValueError, ImportError):
    Gst = None

PIPELINE = ("v4l2src name=src ! decodebin ! videoconvert ! zbar cache=true ! "
            "videoconvert ! gtk4paintablesink name=sink")
PAIR_TIMEOUT = 300      # seconds the pairing code stays valid
PAIR_MAX_BAD = 5        # wrong tokens before the listener closes


def list_cameras(root="/sys/class/video4linux"):
    """[(device, name)] of real capture cameras, lowest number first."""
    cams = []
    for d in glob.glob(os.path.join(root, "video*")):
        base = os.path.basename(d)
        if not base[5:].isdigit():
            continue
        dev = os.path.realpath(os.path.join(d, "device"))
        if "/devices/virtual/" in dev + "/":
            continue          # v4l2loopback and friends
        try:
            with open(os.path.join(d, "index")) as f:
                if f.read().strip() != "0":
                    continue  # metadata node of a camera
        except OSError:
            continue
        try:
            with open(os.path.join(d, "name")) as f:
                name = f.read().strip() or base
        except OSError:
            name = base
        cams.append((int(base[5:]), "/dev/" + base, name))
    return [(dev, name) for _n, dev, name in sorted(cams)]


def parse_phone_qr(text):
    """(location, user, password) for a Harbor phone code, else None."""
    try:
        u = urlsplit(text.strip())
        port = u.port
    except ValueError:
        return None
    if u.scheme != "ftp" or not u.hostname:
        return None
    user = unquote(u.username or "") or "harbor"
    host = "[%s]" % u.hostname if ":" in u.hostname else u.hostname
    loc = "ftp://%s@%s%s/" % (user, host, ":%d" % port if port else "")
    return loc, user, unquote(u.password or "")


def check_pair(token, path, body, client_ip):
    """(http status, ftp code or None) for a phone's POST /pair."""
    if path.split("?")[0] != "/pair":
        return 404, None
    try:
        data = json.loads(body.decode("utf-8"))
        got, ftp = str(data["token"]), str(data["ftp"])
    except (ValueError, KeyError, TypeError, UnicodeDecodeError):
        return 400, None
    if not hmac.compare_digest(got.encode(), token.encode()):
        return 403, None
    if parse_phone_qr(ftp) is None:
        return 400, None
    if urlsplit(ftp.strip()).hostname != client_ip:
        return 403, None      # the share must be on the phone that answered
    return 200, ftp


class PairListener:
    """One-shot HTTP listener for "Scan with the phone". on_pair(ftp) runs in
    the listener's thread; the listener closes after it."""

    def __init__(self, on_pair, host=None):
        if host is None:
            from .files_share import lan_address
            host = lan_address()
        if not host:
            raise OSError("This PC has no Wi-Fi or LAN address")
        self.token = secrets.token_urlsafe(16)
        self.on_pair = on_pair
        self.bad = 0
        self.lock = threading.Lock()
        listener = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_a):  # never log requests (token in body)
                pass

            def do_POST(self):
                listener._handle(self)

        self.srv = ThreadingHTTPServer((host, 0), Handler)
        self.srv.daemon_threads = True
        self.url = "harborpair://%s:%d/%s" % (
            "[%s]" % host if ":" in host else host, self.srv.server_address[1], self.token)
        self.thread = threading.Thread(target=self.srv.serve_forever, daemon=True)
        self.thread.start()

    def _handle(self, h):
        try:
            n = min(int(h.headers.get("Content-Length") or 0), 4096)
        except ValueError:
            n = 0
        code, ftp = check_pair(self.token, h.path, h.rfile.read(n), h.client_address[0])
        with self.lock:
            if code == 403:
                self.bad += 1
            done = ftp is not None and self.on_pair is not None
            cb, self.on_pair = (self.on_pair, None) if done else (None, self.on_pair)
            too_many = self.bad >= PAIR_MAX_BAD
        h.send_response(code if (done or code != 200) else 409)
        h.send_header("Content-Type", "application/json")
        h.end_headers()
        h.wfile.write(b'{"ok":true}' if done else b'{"ok":false}')
        if done:
            cb(ftp)
        if done or too_many:
            threading.Thread(target=self.close, daemon=True).start()

    def close(self):
        srv, self.srv = self.srv, None
        if srv is not None:
            srv.shutdown()
            srv.server_close()


def open_phone(win, text):
    """Mount the phone's FTP share with the scanned PIN and open it in a tab."""
    got = parse_phone_qr(text)
    if got is None:
        win.notify_toast("That is not a Harbor phone code. On the phone open "
                         "Harbor Files › Connect › Start sharing.", timeout=6)
        return
    loc, user, password = got
    op = Gio.MountOperation()
    op.set_username(user)
    op.set_password(password)
    op.set_password_save(Gio.PasswordSave.NEVER)
    op.set_anonymous(False)
    tries = [0]

    def ask(o, *_):
        tries[0] += 1
        o.reply(Gio.MountOperationResult.HANDLED if tries[0] == 1
                else Gio.MountOperationResult.ABORTED)
    op.connect("ask-password", ask)

    def done(f, res):
        try:
            f.mount_enclosing_volume_finish(res)
        except GLib.Error as e:
            if not e.matches(Gio.io_error_quark(), Gio.IOErrorEnum.ALREADY_MOUNTED):
                win.notify_toast("Can't open the phone: " + e.message, timeout=6)
                return
        win.open_location(loc, new_tab=True)
    Gio.File.new_for_uri(loc).mount_enclosing_volume(Gio.MountMountFlags.NONE, op,
                                                     None, done)


def _page_box():
    return Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10,
                   margin_top=12, margin_bottom=18, margin_start=18, margin_end=18)


class ScanDialog(Adw.Dialog):
    def __init__(self, win, page=None):
        super().__init__(title="Open a phone", content_width=560, content_height=560)
        self.win = win
        self.pipe = None
        self.pair = None
        self.pair_timer = 0
        self.cameras = list_cameras()
        self.cam_i = 0
        self.done = False

        tv = Adw.ToolbarView()
        hb = Adw.HeaderBar()
        self.stack = Adw.ViewStack(vexpand=True)
        hb.set_title_widget(Adw.ViewSwitcher(stack=self.stack,
                                             policy=Adw.ViewSwitcherPolicy.WIDE))
        tv.add_top_bar(hb)

        cam = _page_box()
        cam.append(Gtk.Label(label="On the phone open Harbor Files › Connect › Start sharing "
                                   "and hold its QR code up to the camera.", wrap=True))
        self.picture = Gtk.Picture(vexpand=True, content_fit=Gtk.ContentFit.CONTAIN)
        cam.append(self.picture)
        self.status = Gtk.Label(css_classes=["dim-label"], wrap=True)
        cam.append(self.status)
        self.stack.add_titled_with_icon(cam, "camera", "Scan with this PC", "camera-web-symbolic")

        ph = _page_box()
        ph.append(Gtk.Label(label="On the phone open Harbor Files › Connect, tap the scan-QR "
                                  "icon and scan this code. The phone shares its storage and this "
                                  "PC opens it.", wrap=True))
        self.qr_slot = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, vexpand=True,
                               valign=Gtk.Align.CENTER, halign=Gtk.Align.CENTER)
        ph.append(self.qr_slot)
        self.pair_status = Gtk.Label(css_classes=["dim-label"], wrap=True)
        ph.append(self.pair_status)
        self.stack.add_titled_with_icon(ph, "phone", "Scan with the phone",
                                        "phone-symbolic")

        tv.set_content(self.stack)
        self.set_child(tv)
        self.connect("closed", lambda *_: self._stop_all())
        self.stack.connect("notify::visible-child-name", lambda *_: self._page_changed())
        self.stack.set_visible_child_name(page or ("camera" if self.cameras else "phone"))
        self._page_changed()

    # --- pages -------------------------------------------------------------
    def _page_changed(self):
        if self.done:
            return
        if self.stack.get_visible_child_name() == "camera":
            self._start_camera()
        else:
            self._stop_camera()
            self._start_pair()

    def _stop_all(self):
        self.done = True
        self._stop_camera()
        self._stop_pair()

    def _finish(self, ftp):
        self._stop_all()
        self.close()
        open_phone(self.win, ftp)

    # --- scan with this PC ----------------------------------------------------
    def _start_camera(self):
        if self.pipe is not None:
            return
        if Gst is None:
            self.status.set_label("Camera scanning needs GStreamer (gir1.2-gst-plugins-base-1.0).")
            return
        if self.cam_i >= len(self.cameras):
            self.status.set_label("No camera found. Use “Scan with the phone” instead.")
            return
        Gst.init(None)
        dev, name = self.cameras[self.cam_i]
        try:
            self.pipe = Gst.parse_launch(PIPELINE)
            # a property, not text in the launch line: shell quotes stay literal there
            self.pipe.get_by_name("src").set_property("device", dev)
        except GLib.Error as e:
            self.status.set_label("Camera scanning needs the GStreamer zbar and GTK 4 "
                                  "plugins (gstreamer1.0-plugins-bad, gstreamer1.0-gtk4): "
                                  + e.message)
            self.pipe = None
            return
        self.picture.set_paintable(self.pipe.get_by_name("sink").props.paintable)
        bus = self.pipe.get_bus()
        bus.add_signal_watch()
        bus.connect("message", self._message)
        self.pipe.set_state(Gst.State.PLAYING)
        self.status.set_label("Looking for a QR code with %s…" % name)

    def _message(self, _bus, msg):
        if msg.type == Gst.MessageType.ERROR:
            err, _dbg = msg.parse_error()
            self._stop_camera()
            self.cam_i += 1
            if self.cam_i < len(self.cameras):
                self._start_camera()
            else:
                self.status.set_label("No camera: " + err.message +
                                      " Use “Scan with the phone” instead.")
        elif msg.type == Gst.MessageType.ELEMENT:
            st = msg.get_structure()
            if st is not None and st.get_name() == "barcode":
                text = st.get_string("symbol") or ""
                if parse_phone_qr(text) is None:
                    self.status.set_label("That QR code is not from Harbor Files for Android.")
                    return
                self._finish(text)

    def _stop_camera(self):
        if self.pipe is not None:
            self.pipe.get_bus().remove_signal_watch()
            self.pipe.set_state(Gst.State.NULL)
            self.pipe = None

    # --- scan with the phone --------------------------------------------------
    def _start_pair(self):
        if self.pair is not None:
            return
        try:
            self.pair = PairListener(lambda ftp: GLib.idle_add(self._paired, ftp))
        except OSError as e:
            self.pair_status.set_label("Can't wait for the phone: %s." % (e.strerror or e))
            return
        from .files_share_ui import qr_picture
        child = self.qr_slot.get_first_child()
        if child is not None:
            self.qr_slot.remove(child)
        self.qr_slot.append(qr_picture(self.pair.url))
        self.pair_status.set_label("Waiting for the phone… Both must be on the same Wi-Fi. "
                                   "The code works once, for 5 minutes.")
        self.pair_timer = GLib.timeout_add_seconds(PAIR_TIMEOUT, self._pair_expired)

    def _paired(self, ftp):
        self.pair = None          # the listener closes itself
        if not self.done:
            self._finish(ftp)
        return False

    def _pair_expired(self):
        self.pair_timer = 0
        self._stop_pair()
        child = self.qr_slot.get_first_child()
        if child is not None:
            self.qr_slot.remove(child)
        btn = Gtk.Button(label="Show a new code", css_classes=["pill", "suggested-action"])
        btn.connect("clicked", lambda *_: self._start_pair())
        self.qr_slot.append(btn)
        self.pair_status.set_label("The code expired.")
        return False

    def _stop_pair(self):
        if self.pair_timer:
            GLib.source_remove(self.pair_timer)
            self.pair_timer = 0
        if self.pair is not None:
            self.pair.close()
            self.pair = None
