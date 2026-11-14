"""Harbor Files: the navigation pane (Home, Quick access, This PC, drives, Network, Trash)."""

import os

from gi.repository import Gdk, Gio, GLib, GObject, Gtk

from . import files_core as fc
from . import files_network as fn
from . import files_icons
from .common import NavList

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


class SideList(NavList):
    """NavList that can be rebuilt and takes Gio icons and indented rows."""

    def clear(self):
        self.remove_all()
        self.rows = {}

    def add_entry(self, key, text, icon, indent=0, tooltip=None):
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
        self._popover = None
        self.has_network = "network" in Gio.Vfs.get_default().get_supported_uri_schemes()
        self.vm = Gio.VolumeMonitor.get()
        self._rebuild_id = 0
        for sig in ("mount-added", "mount-removed", "mount-changed", "volume-added",
                    "volume-removed", "volume-changed"):
            self.vm.connect(sig, lambda *_: self.queue_rebuild())
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
            out.append(("mount:%d" % i, mount_label(m), m.get_symbolic_icon() or m.get_icon(), m,
                        m.get_root().get_uri()))
        for i, v in enumerate(self.vm.get_volumes()):
            if v in seen_vol or v.get_mount() is not None or not v.can_mount():
                continue
            out.append(("volume:%d" % i, v.get_name() or "Drive",
                        v.get_symbolic_icon() or v.get_icon(), v, None))
        return out

    def rebuild(self):
        lst = self.list
        lst.clear()
        self.targets = {}
        self.objects = {}

        def add(key, text, icon, target, indent=0, tooltip=None):
            lst.add_entry(key, text, icon, indent, tooltip)
            self.targets[key] = target
        add("home", "Home", "go-home-symbolic", fc.HOME_PAGE)
        lst.add_separator()
        for q in quick_access(self.settings):
            add("pin:" + q["path"], q["label"], q["icon"], fc.location_for(q["path"]),
                tooltip=q["path"])
        lst.add_separator()
        add("thispc", "This PC", "computer-symbolic", fc.THIS_PC)
        add("root", "File System", "drive-harddisk-symbolic", fc.location_for("/"), indent=14)
        for key, text, icon, obj, loc in self.drives():
            self.objects[key] = obj
            add(key, text, icon, loc or ("volume", obj), indent=14)
        if self.has_network:
            add("network", "Network", "network-workgroup-symbolic", fc.NETWORK_URI)
        for n in self.settings.network_locations():
            add("net:" + n["url"], fn.display_name(n["url"], n["name"]), fn.icon_for(n["url"]),
                n["url"], indent=14, tooltip=n["url"])
        lst.add_separator()
        add("trash", "Trash", "user-trash-full-symbolic" if self.trash_full
            else "user-trash-symbolic", fc.TRASH_URI)
        if self.current:
            self.sync(self.current)

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
        if key.startswith("mount:"):
            obj = self.objects.get(key)
            if obj is not None and obj.can_eject():
                s2.append_item(mi("Eject", "win.sb-eject", key))
            if obj is not None and obj.can_unmount():
                s2.append_item(mi("Unmount", "win.sb-unmount", key))
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
