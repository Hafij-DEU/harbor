"""Rename a drive: a Harbor-only name (kept in Harbor's settings) or the
filesystem label on the drive itself (UDisks2, seen by every app)."""

import gi

gi.require_version("Adw", "1")
gi.require_version("Gtk", "4.0")
from gi.repository import Adw, Gio, GLib, Gtk  # noqa: E402

UDISKS = "org.freedesktop.UDisks2"
MAX_LEN = 80


def _msg(err):
    """GDBus error text without the 'GDBus.Error:org.x.Y: ' prefix."""
    msg = err.message or ""
    if msg.startswith("GDBus.Error:") and ": " in msg:
        msg = msg.split(": ", 1)[1]
    return msg


def _needs_unmount(msg):
    """ntfslabel, exfatlabel and friends refuse to touch a mounted drive."""
    m = (msg or "").lower()
    return "mounted" in m and ("cannot" in m or "can't" in m or "busy" in m or "must" in m)


def set_system_label(device, label, done):
    """Write the filesystem label of /dev node `device` through UDisks2.
    done(error_text_or_None, remounted_bool). UDisks asks for the password
    itself (polkit). Drives whose label tool refuses to work while mounted
    (NTFS, exFAT) are unmounted, relabelled and mounted again."""
    try:
        bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
    except GLib.Error as e:
        done(e.message, False)
        return
    fs = UDISKS + ".Filesystem"
    state = {"path": None, "retried": False}

    def call(meth, args, sig, cb, timeout=180000):
        bus.call(UDISKS, state["path"], fs, meth, GLib.Variant(sig, args), None,
                 Gio.DBusCallFlags.NONE, timeout, None, cb)

    def set_label():
        call("SetLabel", (label, {}), "(sa{sv})", on_set)

    def on_set(conn, res):
        try:
            conn.call_finish(res)
        except GLib.Error as e:
            msg = _msg(e)
            if not state["retried"] and _needs_unmount(msg):
                state["retried"] = True
                call("Unmount", ({},), "(a{sv})", on_unmount)
                return
            if state["retried"]:
                remount(msg or "Couldn't rename the drive")
                return
            done(msg or "Couldn't rename the drive", False)
            return
        if state["retried"]:
            remount(None)
        else:
            done(None, False)

    def on_unmount(conn, res):
        try:
            conn.call_finish(res)
        except GLib.Error as e:
            msg = _msg(e)
            if "busy" in msg.lower() or "target is busy" in msg.lower():
                msg = "the drive is in use. Close files and apps using it, then try again"
            done(msg, False)
            return
        set_label()

    def remount(err):
        def on_mount(conn, res):
            try:
                conn.call_finish(res)
            except GLib.Error as e:
                done(err or "renamed, but mounting it again failed: %s" % _msg(e), True)
                return
            done(err, True)
        call("Mount", ({},), "(a{sv})", on_mount)

    def on_resolve(conn, res):
        try:
            paths = conn.call_finish(res).unpack()[0]
        except GLib.Error as e:
            done(_msg(e), False)
            return
        if not paths:
            done("UDisks doesn't know this drive", False)
            return
        state["path"] = paths[0]
        set_label()

    bus.call(UDISKS, "/org/freedesktop/UDisks2/Manager", UDISKS + ".Manager", "ResolveDevice",
             GLib.Variant("(a{sv}a{sv})", ({"path": GLib.Variant("s", device)}, {})),
             GLib.VariantType("(ao)"), Gio.DBusCallFlags.NONE, -1, None, on_resolve)


class RenameDriveDialog(Adw.AlertDialog):
    def __init__(self, sidebar, did, current, system_name, device):
        super().__init__(heading="Rename drive")
        self.sidebar, self.did, self.device = sidebar, did, device
        self.has_alias = bool(sidebar.settings.drive_name(did))
        self.add_response("cancel", "Cancel")
        if self.has_alias:
            self.add_response("reset", "Use “%s”" % system_name)
        self.add_response("rename", "Rename")
        self.set_response_appearance("rename", Adw.ResponseAppearance.SUGGESTED)
        self.set_default_response("rename")
        self.set_close_response("cancel")

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        self.entry = Gtk.Entry(text=current, max_length=MAX_LEN, activates_default=True)
        box.append(self.entry)
        self.local = Gtk.CheckButton(active=True)
        self.local.set_child(self._choice("Only in Harbor",
                                          "Sidebar and This PC. Nothing on the drive changes."))
        self.system = Gtk.CheckButton(group=self.local, sensitive=device is not None)
        self.system.set_child(self._choice(
            "Drive label (every app)",
            "Written to the drive. Your password may be asked; NTFS and exFAT drives "
            "are unmounted for a moment." if device else "Not available for this drive."))
        box.append(self.local)
        box.append(self.system)
        self.set_extra_child(box)
        self.entry.connect("changed", lambda *_: self.set_response_enabled(
            "rename", bool(self.entry.get_text().strip())))
        self.connect("response", self._on_response)
        GLib.idle_add(lambda: (self.entry.grab_focus(), self.entry.select_region(0, -1),
                               False)[-1])

    @staticmethod
    def _choice(title, sub):
        col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=1, margin_start=4)
        col.append(Gtk.Label(label=title, xalign=0))
        col.append(Gtk.Label(label=sub, xalign=0, wrap=True, max_width_chars=40,
                             css_classes=["dim-label", "caption"]))
        return col

    def _on_response(self, _dlg, resp):
        sb = self.sidebar
        name = self.entry.get_text().strip()
        if resp == "reset":
            sb.settings.set_drive_name(self.did, "")
            sb.names_changed()
        elif resp == "rename" and name:
            if self.system.get_active() and self.device:
                def done(err, remounted):
                    if remounted:
                        sb.names_changed()
                    if err:
                        sb.win.notify_toast("Couldn't rename: %s" % err)
                        return
                    sb.settings.set_drive_name(self.did, "")
                    sb.names_changed()
                    sb.win.notify_toast("Drive renamed to “%s”" % name)
                set_system_label(self.device, name, done)
            else:
                sb.settings.set_drive_name(self.did, name)
                sb.names_changed()
