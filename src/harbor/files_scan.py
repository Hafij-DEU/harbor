"""Harbor Files: "Scan phone QR" — read Harbor for Android's QR code with the
webcam and open the phone's files over FTP.

The phone's code reads ftp://harbor:PIN@ip:port/. The PIN goes straight into
a Gio.MountOperation for gvfs; it is never logged or saved (gvfs is told
PasswordSave.NEVER). Needs GStreamer with v4l2src, zbar and gtk4paintablesink;
without them the dialog says so.
"""

from urllib.parse import unquote, urlsplit

from gi.repository import Adw, Gio, GLib, Gtk

try:
    import gi
    gi.require_version("Gst", "1.0")
    from gi.repository import Gst
except (ValueError, ImportError):
    Gst = None

PIPELINE = ("v4l2src ! videoconvert ! zbar cache=true ! videoconvert ! "
            "gtk4paintablesink name=sink")


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


def open_phone(win, text):
    """Mount the phone's FTP share with the scanned PIN and open it in a tab."""
    got = parse_phone_qr(text)
    if got is None:
        win.notify_toast("That is not a Harbor phone code. On the phone open "
                         "Harbor Files › Connect › Share this phone.", timeout=6)
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


class ScanDialog(Adw.Dialog):
    def __init__(self, win):
        super().__init__(title="Scan phone QR", content_width=560, content_height=520)
        self.win = win
        self.pipe = None
        tv = Adw.ToolbarView()
        tv.add_top_bar(Adw.HeaderBar())
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10,
                      margin_top=12, margin_bottom=18, margin_start=18, margin_end=18)
        box.append(Gtk.Label(label="On the phone open Harbor Files › Connect and hold its "
                                   "QR code up to the camera.", wrap=True))
        self.picture = Gtk.Picture(vexpand=True, content_fit=Gtk.ContentFit.CONTAIN)
        box.append(self.picture)
        self.status = Gtk.Label(css_classes=["dim-label"], wrap=True)
        box.append(self.status)
        tv.set_content(box)
        self.set_child(tv)
        self.connect("closed", lambda *_: self._stop())
        self._start()

    def _start(self):
        if Gst is None:
            self.status.set_label("Camera scanning needs GStreamer (gir1.2-gst-plugins-base-1.0).")
            return
        Gst.init(None)
        try:
            self.pipe = Gst.parse_launch(PIPELINE)
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
        self.status.set_label("Looking for a QR code…")

    def _message(self, _bus, msg):
        if msg.type == Gst.MessageType.ERROR:
            err, _dbg = msg.parse_error()
            self.status.set_label("No camera: " + err.message)
            self._stop()
        elif msg.type == Gst.MessageType.ELEMENT:
            st = msg.get_structure()
            if st is not None and st.get_name() == "barcode":
                text = st.get_string("symbol") or ""
                if parse_phone_qr(text) is None:
                    self.status.set_label("That QR code is not from Harbor Files for Android.")
                    return
                self._stop()
                self.close()
                open_phone(self.win, text)

    def _stop(self):
        if self.pipe is not None:
            self.pipe.get_bus().remove_signal_watch()
            self.pipe.set_state(Gst.State.NULL)
            self.pipe = None
