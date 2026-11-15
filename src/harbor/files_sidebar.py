"""Harbor Files: the navigation pane (Home, Quick access, This PC, drives, Network, Trash)."""

import os
import re
from urllib.parse import urlsplit

from gi.repository import Gdk, Gio, GLib, GObject, Gtk

from . import files_core as fc
from . import files_network as fn
from . import files_icons
from .common import NavList

PHONE_CHECK_SECS = 5
PHONE_MISSES = 2
PHONE_NAMES = {}    # share uri -> device name the phone sent in its FTP banner


def phone_share_addr(uri):
    """(host, port) of a Harbor phone share (ftp://harbor@ip:port/), else None."""
    try:
        u = urlsplit(uri or "")
        port = u.port
    except ValueError:
        return None
    if u.scheme != "ftp" or u.username != "harbor" or not u.hostname:
        return None
    return u.hostname, port or 21


def parse_banner(line):
    """Device name from the Harbor phone's FTP greeting
    "220 Harbor Files FTP ready (Redmi Pad 2)", else None."""
    m = re.match(r"220[ -]Harbor Files FTP ready \((.{1,60})\)\s*$", line or "")
    return m.group(1).strip() or None if m else None


def phone_label(uri):
    addr = phone_share_addr(uri)
    if addr is None:
        return None
    return PHONE_NAMES.get(uri) or "Phone (%s)" % addr[0]

def user_dirs(home=None, xdg=None):
    """[(path, label, icon)] for the user folders that exist, in Explorer order
    (Desktop, Documents, Downloads, Pictures, Music, Videos). An XDG entry that
    is unset or points at home falls back to ~/<Name> (fc.user_folder)."""
    return [(p, os.path.basename(p.rstrip("/")) or kind, icon)
            for kind, p, icon in fc.user_folders(home, xdg) if os.path.isdir(p)]


def quick_access(settings):
    """Pinned folders: defaults (home + user folders) then the user's pins."""
    home = os.path.normpath(fc.home_dir())
    hidden = set(settings.get("hidden_defaults") or [])
    out = []
    seen = set()
    for path, lab, icon in [(home, "My files", "user-home")] + user_dirs():
        if path not in hidden:
            out.append({"path": path, "label": lab, "icon": icon, "default": True})
            seen.add(path)
    for p in settings.pins:
        if p not in seen and os.path.isdir(p):
            out.append({"path": p, "label": fc.location_title(fc.location_for(p), path=p),
                        "icon": "folder", "default": False})
            seen.add(p)
    return out


def is_pinned(settings, path):
    path = os.path.normpath(path)
    return any(q["path"] == path for q in quick_access(settings))


def pin(settings, path):
    path = os.path.normpath(path)
    hidden = list(settings.get("hidden_defaults") or [])
    if path in hidden:
        hidden.remove(path)
        settings.set("hidden_defaults", hidden)
        return True
    return settings.pin(path)


def pin_dropped(settings, paths):
    """Pin each dropped local folder that isn't in Quick access yet; returns the new pins."""
    added = []
    for p in paths:
        if p and os.path.isdir(p) and not is_pinned(settings, p):
            pin(settings, p)
            added.append(os.path.normpath(p))
    return added


def unpin(settings, path):
    path = os.path.normpath(path)
    defaults = {os.path.normpath(fc.home_dir())} | {p for p, _l, _i in user_dirs()}
    if path in defaults:
        hidden = list(settings.get("hidden_defaults") or [])
        if path not in hidden:
            hidden.append(path)
            settings.set("hidden_defaults", hidden)
        return True
    return settings.unpin(path)


def mount_label(mount):
    return mount.get_name() or "Drive"


def drive_volume(obj):
    if obj is None:
        return None
    return obj if isinstance(obj, Gio.Volume) else obj.get_volume()


def drive_id(obj):
    """Stable key for a drive's Harbor-only name: the filesystem UUID when known,
    else the device node or mount root. "root" is the File System entry."""
    if obj is None:
        return "root"
    vol = drive_volume(obj)
    uuid = (vol.get_uuid() if vol is not None else None) or (
        obj.get_uuid() if isinstance(obj, Gio.Mount) else None)
    if uuid:
        return "uuid:" + uuid
    dev = vol.get_identifier("unix-device") if vol is not None else None
    if dev:
        return "dev:" + dev
    if isinstance(obj, Gio.Mount):
        return "loc:" + obj.get_root().get_uri()
    return None


def drive_device(obj):
    """/dev node of a drive (None = not a local block device)."""
    if obj is None:
        try:
            with open("/proc/self/mounts", encoding="utf-8") as f:
                for line in f:
                    parts = line.split()
                    if len(parts) > 2 and parts[1] == "/" and parts[0].startswith("/dev/"):
                        return parts[0]
        except OSError:
            pass
        return None
    vol = drive_volume(obj)
    dev = vol.get_identifier("unix-device") if vol is not None else None
    return dev if dev and dev.startswith("/dev/") else None


class SideList(NavList):
    """NavList that can be rebuilt and takes Gio icons and indented rows."""

    def clear(self):
        self.remove_all()
        self.rows = {}

    def add_entry(self, key, text, icon, indent=0, tooltip=None, pinned=False, end=None):
        box = Gtk.Box(spacing=10, margin_start=4 + indent, margin_end=10)
        box.append(Gtk.Box(css_classes=["hb-nav-pill"], valign=Gtk.Align.CENTER))
        img = Gtk.Image(pixel_size=18)
        if isinstance(icon, Gio.Icon):
            img.set_from_gicon(files_icons.themed_gicon(icon))
        else:
            img.set_from_icon_name(files_icons.themed(icon))
        box.append(img)
        lab = Gtk.Label(label=text, xalign=0, hexpand=True, css_classes=["hb-nav-label"],
                        ellipsize=3)
        box.append(lab)
        if pinned:
            box.append(Gtk.Image(icon_name="view-pin-symbolic", pixel_size=12,
                                 css_classes=["hbf-pin-mark"]))
        if end is not None:     # (icon, tooltip, callback): e.g. Disconnect
            b = Gtk.Button(icon_name=end[0], tooltip_text=end[1], valign=Gtk.Align.CENTER,
                           css_classes=["flat", "circular", "hbf-side-end"])
            b.connect("clicked", lambda *_: end[2]())
            box.append(b)
        row = Gtk.ListBoxRow(child=box)
        row.key = key
        row.label = lab
        if tooltip:
            row.set_tooltip_text(tooltip)
        self.append(row)
        self.rows[key] = row
        return row

    def add_separator(self):
        row = Gtk.ListBoxRow(selectable=False, activatable=False, can_focus=False,
                             css_classes=["hbf-side-sep-row"])
        row.set_child(Gtk.Separator(margin_start=14, margin_end=14, valign=Gtk.Align.CENTER,
                                    css_classes=["hbf-sep"]))
        row.key = None
        self.append(row)


class Sidebar(Gtk.Box):
    """Emits open(location, new_tab) and menu(x, y, key)."""

    __gsignals__ = {
        "open": (GObject.SignalFlags.RUN_FIRST, None, (str, bool)),
    }

    def __init__(self, win):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, css_classes=["hbf-sidebar"],
                         width_request=220)
        self.win = win
        self.settings = win.app.settings
        self.targets = {}       # row key -> location | ("volume", Gio.Volume)
        self.objects = {}       # row key -> Gio.Mount / Gio.Volume
        self.trash_full = False
        self.list = SideList()
        self.list.set_margin_top(4)
        self.list.connect("selected", self._on_selected)
        sw = Gtk.ScrolledWindow(child=self.list, vexpand=True)
        sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.append(sw)
        click = Gtk.GestureClick(button=0)
        click.connect("pressed", self._on_click)
        self.list.add_controller(click)
        drop = Gtk.DropTarget.new(Gdk.FileList, Gdk.DragAction.COPY | Gdk.DragAction.LINK)
        drop.connect("drop", self._on_drop)
        self.list.add_controller(drop)
        self._popover = None
        self.has_network = "network" in Gio.Vfs.get_default().get_supported_uri_schemes()
        self.vm = Gio.VolumeMonitor.get()
        self._rebuild_id = 0
        for sig in ("mount-added", "mount-removed", "mount-changed", "volume-added",
                    "volume-removed", "volume-changed"):
            self.vm.connect(sig, lambda *_: self.queue_rebuild())
        self._phone_miss = {}    # phone share uri -> failed reach checks in a row
        GLib.timeout_add_seconds(PHONE_CHECK_SECS, self._check_phones)
        self._trash_mon = None
        try:
            self._trash_mon = Gio.File.new_for_uri(fc.TRASH_URI).monitor_directory(
                Gio.FileMonitorFlags.NONE, None)
            self._trash_mon.set_rate_limit(500)
            self._trash_mon.connect("changed", lambda *_: self.queue_trash_update())
        except GLib.Error:
            pass
        self.current = None
        self.rebuild()
        self.update_trash()

    # ---------------------------------------------------------- building
    def queue_rebuild(self):
        if not self._rebuild_id:
            self._rebuild_id = GLib.timeout_add(150, self._do_rebuild)

    def _do_rebuild(self):
        self._rebuild_id = 0
        self.rebuild()
        return False

    def drives(self):
        """[(key, label, icon, mount_or_volume, location_or_None)] for This PC."""
        out = []
        seen_vol = set()
        for i, m in enumerate(self.vm.get_mounts()):
            if m.is_shadowed():
                continue
            vol = m.get_volume()
            if vol is not None:
                seen_vol.add(vol)
            uri = m.get_root().get_uri()
            phone = phone_label(uri)
            if phone:
                out.append(("mount:%d" % i, phone, Gio.ThemedIcon.new("phone-symbolic"), m, uri))
                continue
            out.append(("mount:%d" % i, self.drive_label(m), m.get_symbolic_icon() or m.get_icon(),
                        m, uri))
        for i, v in enumerate(self.vm.get_volumes()):
            if v in seen_vol or v.get_mount() is not None or not v.can_mount():
                continue
            out.append(("volume:%d" % i, self.drive_label(v),
                        v.get_symbolic_icon() or v.get_icon(), v, None))
        return out

    def drive_label(self, obj):
        """Harbor-only name if one is set, else the system name."""
        alias = self.settings.drive_name(drive_id(obj))
        if alias:
            return alias
        if obj is None:
            return "File System"
        if isinstance(obj, Gio.Mount):
            return mount_label(obj)
        return obj.get_name() or "Drive"

    def rename_drive(self, key):
        obj = None if key == "root" else self.objects.get(key)
        if key != "root" and obj is None:
            return
        did = drive_id(obj)
        if did is None:
            self.win.notify_toast("This drive can't be renamed")
            return
        from .files_drive_rename import RenameDriveDialog
        system = ("File System" if obj is None else mount_label(obj)
                  if isinstance(obj, Gio.Mount) else obj.get_name() or "Drive")
        RenameDriveDialog(self, did, self.drive_label(obj), system,
                          drive_device(obj)).present(self.win)

    def names_changed(self):
        self.rebuild()
        self.win.lookup_action("refresh").activate(None)

    def rebuild(self):
        lst = self.list
        lst.clear()
        self.targets = {}
        self.objects = {}

        def add(key, text, icon, target, indent=0, tooltip=None, pinned=False, end=None):
            lst.add_entry(key, text, icon, indent, tooltip, pinned, end)
            self.targets[key] = target
        add("home", "Home", "go-home-symbolic", fc.HOME_PAGE)
        lst.add_separator()
        for q in quick_access(self.settings):
            add("pin:" + q["path"], q["label"], q["icon"], fc.location_for(q["path"]),
                tooltip=q["path"], pinned=not q["default"])
        lst.add_separator()
        add("thispc", "This PC", "computer-symbolic", fc.THIS_PC)
        add("root", self.drive_label(None), "drive-harddisk-symbolic", fc.location_for("/"), indent=14)
        for key, text, icon, obj, loc in self.drives():
            self.objects[key] = obj
            phone = loc and phone_share_addr(loc)
            add(key, text, icon, loc or ("volume", obj), indent=14,
                tooltip="Drop files here to send them to the phone" if phone else None,
                end=("media-eject-symbolic", "Disconnect",
                     lambda k=key: self.unmount(k)) if phone else None)
        if self.has_network:
            add("network", "Network", "network-workgroup-symbolic", fc.NETWORK_URI)
        for n in self.settings.network_locations():
            add("net:" + n["url"], fn.display_name(n["url"], n["name"]), fn.icon_for(n["url"]),
                n["url"], indent=14, tooltip=n["url"])
        if self.settings.get("share_in_sidebar"):
            add("wifi-share", "Share over Wi-Fi", "network-wireless-symbolic", ("action", "share"),
                tooltip="Share files with your phone, show the QR code or scan the phone's code")
        lst.add_separator()
        add("trash", "Trash", "user-trash-full-symbolic" if self.trash_full
            else "user-trash-symbolic", fc.TRASH_URI)
        if self.current:
            self.sync(self.current)

    def _on_drop(self, _target, value, _x, y):
        """Files dropped on a phone row are sent to the phone; folders dropped
        anywhere else on the navigation pane get pinned to Quick access."""
        row = self.list.get_row_at_y(int(y))
        loc = self.targets.get(getattr(row, "key", None)) if row is not None else None
        if isinstance(loc, str) and phone_share_addr(loc):
            dest = Gio.File.new_for_uri(loc).get_path()
            paths = [f.get_path() for f in value.get_files() if f.get_path()]
            if not dest or not paths:
                self.win.notify_toast("Can't send these files to the phone")
                return False
            self.win.ops.transfer(paths, dest, move=False)
            self.win.notify_toast("Sending %s to %s" % (
                os.path.basename(paths[0]) if len(paths) == 1 else "%d items" % len(paths),
                row.label.get_label()))
            return True
        added = pin_dropped(self.settings, [f.get_path() for f in value.get_files()])
        if not added:
            return False
        self.rebuild()
        self.win.notify_toast("Pinned %s" % os.path.basename(added[0]) if len(added) == 1
                              else "Pinned %d folders" % len(added))
        return True

    def queue_trash_update(self):
        # emptying a big trash fires one event per item: coalesce them
        if not getattr(self, "_trash_id", 0):
            def run():
                self._trash_id = 0
                self.update_trash()
                return False
            self._trash_id = GLib.timeout_add(300, run)

    def update_trash(self):
        g = Gio.File.new_for_uri(fc.TRASH_URI)

        def done(f, res):
            try:
                info = f.query_info_finish(res)
                full = info.get_attribute_uint32("trash::item-count") > 0
            except GLib.Error:
                return
            if full != self.trash_full:
                self.trash_full = full
                row = self.list.rows.get("trash")
                if row is not None:
                    img = row.get_child().get_first_child().get_next_sibling()
                    img.set_from_icon_name(files_icons.themed(
                        "user-trash-full-symbolic" if full else "user-trash-symbolic"))
        g.query_info_async("trash::item-count", Gio.FileQueryInfoFlags.NONE,
                           GLib.PRIORITY_DEFAULT, None, done)

    # ---------------------------------------------------------- selection
    def sync(self, location):
        """Highlight the row for the location shown in the current tab."""
        self.current = location
        key = None
        for k, t in self.targets.items():
            if t == location:
                key = k
                if k.startswith("pin:") or k in ("home", "trash", "thispc", "network"):
                    break
        self.list.select(key)

    def _on_selected(self, _l, key):
        target = self.targets.get(key)
        if isinstance(target, tuple) and target[0] == "action":
            # not a place: open the share window and keep the current row lit
            self.win.share_wifi()
            GLib.idle_add(lambda: self.sync(self.current) or False)
            return
        if isinstance(target, tuple):
            self.mount_volume(target[1], new_tab=False)
            return
        if target:
            self.emit("open", target, False)

    def mount_volume(self, vol, new_tab=False):
        op = Gtk.MountOperation(parent=self.win)

        def done(v, res):
            try:
                v.mount_finish(res)
            except GLib.Error as e:
                if not e.matches(Gio.io_error_quark(), Gio.IOErrorEnum.FAILED_HANDLED):
                    self.win.notify_toast("Can't open %s: %s" % (v.get_name(), e.message))
                return
            m = v.get_mount()
            if m is not None:
                self.emit("open", m.get_root().get_uri(), new_tab)
        vol.mount(Gio.MountMountFlags.NONE, op, None, done)

    def _check_phones(self):
        """Drop a Harbor phone share once the phone stops answering, so a
        disconnected phone does not linger under This PC."""
        if self.get_root() is None:     # window closed
            return False
        live = set()
        for m in self.vm.get_mounts():
            uri = m.get_root().get_uri()
            addr = phone_share_addr(uri)
            if addr is None:
                continue
            live.add(uri)
            client = Gio.SocketClient(timeout=3)

            def done(c, res, m=m, uri=uri):
                try:
                    conn = c.connect_finish(res)
                    self._phone_miss[uri] = 0
                    if uri in PHONE_NAMES:
                        conn.close(None)
                    else:
                        self._read_name(conn, uri)
                    return
                except GLib.Error:
                    pass
                self._phone_miss[uri] = self._phone_miss.get(uri, 0) + 1
                if self._phone_miss[uri] >= PHONE_MISSES:
                    self._phone_miss.pop(uri, None)
                    self._drop_phone(m, uri)
            client.connect_async(Gio.NetworkAddress.new(*addr), None, done)
        for uri in list(self._phone_miss):
            if uri not in live:
                del self._phone_miss[uri]
        return True

    def _read_name(self, conn, uri):
        """Read the phone's greeting once to learn its name (0.1.4+ phones
        send "(Model)" in it); older phones keep the "Phone (ip)" label."""
        data = Gio.DataInputStream.new(conn.get_input_stream())

        def got(d, res):
            try:
                line, _n = d.read_line_finish_utf8(res)
            except GLib.Error:
                line = None
            conn.close(None)
            name = parse_banner(line)
            PHONE_NAMES[uri] = name or ""
            if name:
                self.queue_rebuild()
                pages = getattr(self.win, "refresh_thispc", None)
                if pages:
                    pages()
        data.read_line_async(GLib.PRIORITY_DEFAULT, None, got)

    def _drop_phone(self, mount, uri):
        for tab in self.win.tabs():
            if isinstance(tab.location, str) and tab.location.startswith(uri.rstrip("/")):
                tab.navigate(fc.THIS_PC)

        def done(o, res):
            try:
                o.unmount_with_operation_finish(res)
            except GLib.Error:
                return
            self.win.notify_toast("Phone disconnected")
        mount.unmount_with_operation(Gio.MountUnmountFlags.FORCE, None, None, done)

    def unmount(self, key, eject=False):
        obj = self.objects.get(key)
        if obj is None:
            return
        op = Gtk.MountOperation(parent=self.win)
        name = obj.get_name()

        def done(o, res):
            try:
                if eject:
                    o.eject_with_operation_finish(res)
                else:
                    o.unmount_with_operation_finish(res)
                self.win.notify_toast("%s can now be removed" % name if eject
                                      else "%s was unmounted" % name)
            except GLib.Error as e:
                if not e.matches(Gio.io_error_quark(), Gio.IOErrorEnum.FAILED_HANDLED):
                    self.win.notify_toast("Can't %s %s: %s" % ("eject" if eject else "unmount",
                                                              name, e.message), timeout=6)
        if eject:
            obj.eject_with_operation(Gio.MountUnmountFlags.NONE, op, None, done)
        elif isinstance(obj, Gio.Mount):
            obj.unmount_with_operation(Gio.MountUnmountFlags.NONE, op, None, done)

    # ---------------------------------------------------------- context menu
    def _on_click(self, gesture, _n, x, y):
        button = gesture.get_current_button()
        if button not in (2, 3):
            return
        row = self.list.get_row_at_y(int(y))
        if row is None or not row.key:
            return
        gesture.set_state(Gtk.EventSequenceState.CLAIMED)
        key = row.key
        target = self.targets.get(key)
        if isinstance(target, tuple) and target[0] == "action":
            self._on_selected(None, key)
            return
        if button == 2:
            if isinstance(target, tuple):
                self.mount_volume(target[1], new_tab=True)
            elif target:
                self.emit("open", target, True)
            return
        menu = self.menu_for(key)
        if menu is None:
            return
        if self._popover is None:
            self._popover = Gtk.PopoverMenu(has_arrow=False, halign=Gtk.Align.START,
                                           flags=Gtk.PopoverMenuFlags.NESTED)
            self._popover.set_parent(self.list)
        self._popover.set_menu_model(menu)
        rect = Gdk.Rectangle()
        rect.x, rect.y, rect.width, rect.height = int(x), int(y), 1, 1
        self._popover.set_pointing_to(rect)
        self._popover.popup()

    def menu_for(self, key):
        from .files_menus import item as mi
        target = self.targets.get(key)
        m = Gio.Menu()
        s1 = Gio.Menu()
        if isinstance(target, str):
            s1.append_item(mi("Open", "win.open-location", target))
            s1.append_item(mi("Open in new tab", "win.open-location-tab", target))
        else:
            s1.append_item(mi("Mount", "win.sb-mount", key))
        m.append_section(None, s1)
        s2 = Gio.Menu()
        if key.startswith("pin:"):
            s2.append_item(mi("Unpin from Quick access", "win.unpin", key[4:]))
            s2.append_item(mi("Open in Terminal", "win.terminal-at", key[4:]))
        if key.startswith(("mount:", "volume:")) and drive_id(self.objects.get(key)) \
                and not phone_share_addr(self.targets.get(key) or ""):
            s2.append_item(mi("Rename…", "win.drive-rename", key))
        if key == "root":
            s2.append_item(mi("Rename…", "win.drive-rename", "root"))
        if key.startswith("mount:"):
            obj = self.objects.get(key)
            if obj is not None and obj.can_eject():
                s2.append_item(mi("Eject", "win.sb-eject", key))
            if obj is not None and obj.can_unmount():
                phone = phone_share_addr(self.targets.get(key) or "")
                s2.append_item(mi("Disconnect" if phone else "Unmount", "win.sb-unmount", key))
        if key == "trash":
            s2.append_item(mi("Empty trash", "win.empty-trash"))
        if key == "network" or key.startswith("net:"):
            s2.append_item(mi("Add network location…", "win.add-network"))
        if key.startswith("net:"):
            s2.append_item(mi("Edit…", "win.net-edit", key[4:]))
            s2.append_item(mi("Remove from sidebar", "win.net-remove", key[4:]))
        if s2.get_n_items():
            m.append_section(None, s2)
        if isinstance(target, str) and target.startswith("file:"):
            s3 = Gio.Menu()
            s3.append_item(mi("Properties", "win.properties-at", target))
            m.append_section(None, s3)
        return m
