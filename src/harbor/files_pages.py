"""Harbor Files: Home and This PC pages, the details pane, Properties and
the "Open with" app picker."""

import os
import threading

from gi.repository import Adw, Gdk, Gio, GLib, Gtk, Pango

from . import files_core as fc
from . import files_menus as fm
from . import files_sidebar as fsb
from .common import greeting, human_size, user_display_name

INFO_ATTRS = ",".join((
    "standard::*", "time::modified", "time::created", "time::access", "unix::mode",
    "owner::user", "owner::group", "thumbnail::path", "trash::orig-path",
    "trash::deletion-date", "access::can-write", "filesystem::size", "filesystem::free",
))


def _label(text="", *classes, xalign=0.0, wrap=False, ellipsize=True, selectable=False, **kw):
    lab = Gtk.Label(label=text, xalign=xalign, css_classes=list(classes), wrap=wrap,
                    selectable=selectable, **kw)
    if wrap:
        lab.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
    elif ellipsize:
        lab.set_ellipsize(Pango.EllipsizeMode.END)
    return lab


def _image(icon, size):
    img = Gtk.Image(pixel_size=size)
    if isinstance(icon, Gio.Icon):
        img.set_from_gicon(icon)
    elif icon:
        img.set_from_icon_name(icon)
    return img


def pretty_dir(path, home=None):
    """"My files › Documents" style text for where a folder lives."""
    home = os.path.normpath(home or fc.home_dir())
    if not path:
        return ""
    path = os.path.normpath(path)
    if path == home:
        return "My files"
    if fc.is_inside(path, home):
        return "My files › " + " › ".join(path[len(home):].strip("/").split("/"))
    return path


def _date_text(ts):
    return fc.format_time(ts) if ts else ""


def path_of(location):
    if not location:
        return None
    if location.startswith("/"):
        return location
    return Gio.File.new_for_uri(location).get_path()


class _Clicks:
    """Right/middle click helper for a FlowBox or ListBox of tiles."""

    def __init__(self, widget, picker, on_menu, on_middle):
        self.widget, self.picker = widget, picker
        g = Gtk.GestureClick(button=0)
        g.connect("pressed", self._pressed, on_menu, on_middle)
        widget.add_controller(g)

    def _pressed(self, gesture, _n, x, y, on_menu, on_middle):
        b = gesture.get_current_button()
        if b not in (2, 3):
            return
        child = self.picker(x, y)
        if child is None:
            return
        gesture.set_state(Gtk.EventSequenceState.CLAIMED)
        if b == 3:
            on_menu(child, x, y)
        else:
            on_middle(child)


def tile(icon, title, sub=None, extra=None, icon_size=40):
    box = Gtk.Box(spacing=12, css_classes=["hbf-qtile"])
    box.append(_image(icon, icon_size))
    col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2, valign=Gtk.Align.CENTER,
                  hexpand=True)
    col.append(_label(title, "hbf-qtile-title"))
    if extra is not None:
        col.append(extra)
    sub_lab = _label(sub or "", "hb-dim", "hb-small")
    sub_lab.set_visible(bool(sub) or extra is not None)
    col.append(sub_lab)
    box.append(col)
    box.sub_label = sub_lab
    return box


def tile_flow(min_w=230):
    fb = Gtk.FlowBox(selection_mode=Gtk.SelectionMode.SINGLE, activate_on_single_click=False,
                     homogeneous=True, max_children_per_line=6, min_children_per_line=1,
                     column_spacing=8, row_spacing=8, css_classes=["hbf-tiles"],
                     valign=Gtk.Align.START)
    fb.min_w = min_w
    return fb


def section_label(text):
    return _label(text, "hbf-section")


# ---------------------------------------------------------------------- Home

class HomePage(Gtk.ScrolledWindow):
    def __init__(self, win):
        super().__init__(vexpand=True, hexpand=True, css_classes=["hbf-page"])
        self.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.win = win
        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6, margin_start=28,
                        margin_end=28, margin_top=22, margin_bottom=24)
        self.hello = _label("", "hbf-hello")
        self.date = _label("", "hb-dim")
        outer.append(self.hello)
        outer.append(self.date)
        outer.append(section_label("Quick access"))
        self.tiles = tile_flow()
        self.tiles.connect("child-activated", self._tile_activated)
        _Clicks(self.tiles, lambda x, y: self.tiles.get_child_at_pos(x, y),
                self._tile_menu, lambda c: win.open_location(c.location, new_tab=True))
        outer.append(self.tiles)
        outer.append(section_label("Recent"))
        head = Gtk.Box(spacing=12, css_classes=["hbf-recent-head"])
        head.append(_label("Name", "hb-dim", "hb-small", hexpand=True))
        head.append(_label("Location", "hb-dim", "hb-small", width_request=300))
        head.append(_label("Date opened", "hb-dim", "hb-small", width_request=140))
        self.recent_head = head
        outer.append(head)
        self.recent = Gtk.ListBox(css_classes=["hbf-recent"], activate_on_single_click=False,
                                  selection_mode=Gtk.SelectionMode.SINGLE)
        self.recent.connect("row-activated", lambda _l, r: win.open_uri(r.uri))
        _Clicks(self.recent, lambda x, y: self.recent.get_row_at_y(int(y)),
                lambda r, x, y: win.popup_menu(self.recent, x, y, fm.recent_menu(r.uri)),
                lambda r: None)
        outer.append(self.recent)
        self.recent_empty = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6,
                                    margin_top=26, halign=Gtk.Align.CENTER)
        self.recent_empty.append(_image("document-open-recent-symbolic", 40))
        self.recent_empty.append(_label("Files you open will show up here", "hb-dim",
                                        xalign=0.5))
        self.recent_empty.get_first_child().add_css_class("hb-faint")
        outer.append(self.recent_empty)
        self.set_child(outer)
        self.rm = Gtk.RecentManager.get_default()
        self._rid = 0
        self._rm_handler = self.rm.connect("changed", lambda *_: self.queue_refresh())
        self.connect("destroy", lambda *_: self.rm.disconnect(self._rm_handler))
        self.refresh()

    def queue_refresh(self):
        if not self._rid:
            self._rid = GLib.timeout_add(250, self._do_refresh)

    def _do_refresh(self):
        self._rid = 0
        self.refresh()
        return False

    def refresh(self):
        now = GLib.DateTime.new_now_local()
        self.hello.set_label("%s, %s" % (greeting(), user_display_name()))
        self.date.set_label(now.format("%A, %-d %B %Y") or "")
        self.tiles.remove_all()
        for q in fsb.quick_access(self.win.app.settings):
            sub = "Pinned" if not q["default"] else (
                "Stored locally" if q["path"] == os.path.normpath(fc.home_dir())
                else pretty_dir(os.path.dirname(q["path"])))
            t = tile(q["icon"].replace("-symbolic", ""), q["label"], sub)
            child = Gtk.FlowBoxChild(child=t)
            child.location = fc.location_for(q["path"])
            child.path = q["path"]
            child.set_tooltip_text(q["path"])
            self.tiles.append(child)
        self.recent.remove_all()
        items = []
        for info in self.rm.get_items():
            uri = info.get_uri()
            if not uri.startswith("file:"):
                continue
            path = Gio.File.new_for_uri(uri).get_path()
            if not path or not os.path.exists(path):
                continue
            dt = info.get_visited() or info.get_modified()
            items.append((dt.to_unix() if dt else 0, uri, path, info))
        items.sort(key=lambda t: t[0], reverse=True)
        for ts, uri, path, info in items[:25]:
            row = Gtk.ListBoxRow()
            box = Gtk.Box(spacing=12, css_classes=["hbf-recent-row"])
            icon = info.get_gicon() or Gio.ThemedIcon.new("text-x-generic")
            box.append(_image(icon, 20))
            box.append(_label(info.get_display_name() or os.path.basename(path), hexpand=True))
            box.append(_label(pretty_dir(os.path.dirname(path)), "hb-dim", width_request=300))
            box.append(_label(_date_text(ts), "hb-dim", width_request=140))
            row.set_child(box)
            row.uri = uri
            row.set_tooltip_text(path)
            self.recent.append(row)
        has = bool(items)
        self.recent.set_visible(has)
        self.recent_head.set_visible(has)
        self.recent_empty.set_visible(not has)

    def _tile_activated(self, _fb, child):
        self.win.open_location(child.location)

    def _tile_menu(self, child, x, y):
        self.tiles.select_child(child)
        pinned = fsb.is_pinned(self.win.app.settings, child.path)
        self.win.popup_menu(self.tiles, x, y, fm.path_menu(child.location, pinned))


# ---------------------------------------------------------------------- This PC

class ThisPCPage(Gtk.ScrolledWindow):
    def __init__(self, win):
        super().__init__(vexpand=True, hexpand=True, css_classes=["hbf-page"])
        self.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.win = win
        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6, margin_start=28,
                        margin_end=28, margin_top=18, margin_bottom=24)
        outer.append(section_label("Folders"))
        self.folders = tile_flow()
        self.folders.connect("child-activated", lambda _f, c: win.open_location(c.location))
        _Clicks(self.folders, lambda x, y: self.folders.get_child_at_pos(x, y),
                self._folder_menu, lambda c: win.open_location(c.location, new_tab=True))
        outer.append(self.folders)
        outer.append(section_label("Devices and drives"))
        self.drives = tile_flow(min_w=280)
        self.drives.connect("child-activated", self._drive_activated)
        _Clicks(self.drives, lambda x, y: self.drives.get_child_at_pos(x, y),
                self._drive_menu, lambda c: c.location and win.open_location(c.location, True))
        outer.append(self.drives)
        self.set_child(outer)
        self.refresh()

    def refresh(self):
        self.folders.remove_all()
        home = os.path.normpath(fc.home_dir())
        for path, lab, icon in [(home, "My files", "user-home")] + fsb.user_dirs():
            child = Gtk.FlowBoxChild(child=tile(icon, lab, "Stored locally" if path == home
                                                else pretty_dir(os.path.dirname(path))))
            child.location = fc.location_for(path)
            child.path = path
            child.set_tooltip_text(path)
            self.folders.append(child)
        self.drives.remove_all()
        entries = [("root", "File System", Gio.ThemedIcon.new("drive-harddisk"), None,
                    fc.location_for("/"))]
        for key, text, icon, obj, loc in self.win.sidebar.drives():
            big = obj.get_icon() if obj is not None else icon
            entries.append((key, text, big, obj, loc))
        for key, text, icon, obj, loc in entries:
            bar = Gtk.ProgressBar(css_classes=["hbf-bar"], fraction=0.0)
            t = tile(icon, text, "Not mounted" if loc is None else "", extra=bar)
            bar.set_visible(loc is not None)
            child = Gtk.FlowBoxChild(child=t)
            child.location, child.obj, child.key = loc, obj, key
            self.drives.append(child)
            if loc is not None:
                self._fill_space(Gio.File.new_for_uri(loc), bar, t.sub_label)

    def _fill_space(self, gfile, bar, sub):
        def done(f, res):
            try:
                info = f.query_filesystem_info_finish(res)
            except GLib.Error:
                bar.set_visible(False)
                return
            size = info.get_attribute_uint64("filesystem::size")
            free = info.get_attribute_uint64("filesystem::free")
            if not size:
                bar.set_visible(False)
                return
            frac = max(0.0, min(1.0, (size - free) / size))
            bar.set_fraction(frac)
            if frac >= 0.9:
                bar.add_css_class("hbf-bar-warn")
            sub.set_label("%s free of %s" % (human_size(free), human_size(size)))
            sub.set_visible(True)
        gfile.query_filesystem_info_async("filesystem::size,filesystem::free",
                                          GLib.PRIORITY_DEFAULT, None, done)

    def _drive_activated(self, _f, child):
        if child.location:
            self.win.open_location(child.location)
        elif child.obj is not None:
            self.win.sidebar.mount_volume(child.obj)

    def _folder_menu(self, child, x, y):
        self.folders.select_child(child)
        pinned = fsb.is_pinned(self.win.app.settings, child.path)
        self.win.popup_menu(self.folders, x, y, fm.path_menu(child.location, pinned))

    def _drive_menu(self, child, x, y):
        self.drives.select_child(child)
        if child.key == "root" or not child.key.startswith(("mount:", "volume:")):
            m = fm.path_menu(child.location, True, can_unpin=False)
        else:
            m = self.win.sidebar.menu_for(child.key)
        self.win.popup_menu(self.drives, x, y, m)


# ---------------------------------------------------------------------- details pane

class DetailsPane(Gtk.ScrolledWindow):
    def __init__(self, win):
        super().__init__(width_request=290, css_classes=["hbf-pane"])
        self.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.win = win
        self._gen = 0
        self.box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4, margin_start=18,
                           margin_end=18, margin_top=16, margin_bottom=16)
        self.set_child(self.box)

    def _clear(self):
        self._gen += 1
        while (c := self.box.get_first_child()) is not None:
            self.box.remove(c)

    def _row(self, key, value):
        if value in (None, ""):
            return None
        self.box.append(_label(key, "hb-dim", "hb-small", margin_top=8))
        v = _label(str(value), "hbf-pane-value", wrap=True, selectable=True)
        self.box.append(v)
        return v

    def _hero(self, icon, title, sub=None):
        img = _image(icon, 96)
        img.set_halign(Gtk.Align.CENTER)
        img.set_margin_top(8)
        img.set_margin_bottom(10)
        self.box.append(img)
        self.box.append(_label(title, "hbf-pane-title", wrap=True, xalign=0.5,
                               justify=Gtk.Justification.CENTER))
        if sub:
            self.box.append(_label(sub, "hb-dim", xalign=0.5))
        self.box.append(Gtk.Separator(margin_top=12, margin_bottom=4, css_classes=["hbf-sep"]))
        return img

    def update(self, tab):
        self._clear()
        if tab is None:
            return
        kind = tab.kind()
        if kind in ("home", "thispc"):
            self._hero("help-about-symbolic" if kind == "home" else "computer",
                       "Home" if kind == "home" else "This PC")
            self.box.append(_label("Select a file or folder to see its details.", "hb-dim",
                                   wrap=True, xalign=0.5, justify=Gtk.Justification.CENTER))
            return
        sel = tab.view.selected_items()
        if len(sel) == 1:
            self._show_item(sel[0])
        elif len(sel) > 1:
            files = [i for i in sel if not i.is_dir]
            dirs = len(sel) - len(files)
            self._hero("edit-select-all-symbolic", "%d items selected" % len(sel))
            self._row("Files", str(len(files)) if files else None)
            self._row("Folders", str(dirs) if dirs else None)
            if files:
                self._row("Total size of files", human_size(sum(i.size for i in files)))
        else:
            loc = tab.location
            title = fc.location_title(loc)
            icon = "user-trash" if kind == "trash" else "folder"
            self._hero(icon, title)
            self._row("Items", fc.item_count_text(len(tab.view.items())))
            p = path_of(loc)
            self._row("Location", p or fc.uri_to_label(loc))
            if tab.search_query:
                self._row("Search", "“%s”" % tab.search_query)

    def _show_item(self, item):
        icon = item.icon or Gio.ThemedIcon.new("text-x-generic")
        img = self._hero(icon, item.name)
        if item.is_image and item.path:
            from .files_view import _THUMBS, _load_thumb
            gen = self._gen
            fut = _THUMBS.submit(_load_thumb, item, 256)

            def put(fu):
                try:
                    tex = fu.result()
                except Exception:
                    tex = None
                if tex is not None and gen == self._gen:
                    img.set_from_paintable(tex)
                    img.set_pixel_size(200)
                return False
            fut.add_done_callback(lambda fu: GLib.idle_add(put, fu))
        self._row("Type", item.type_desc)
        if not item.is_dir:
            self._row("Size", "%s (%s bytes)" % (human_size(item.size), "{:,}".format(item.size)))
        else:
            count = self._row("Contains", "Counting…")
            if item.path and count is not None:
                gen = self._gen

                def worker(path=item.path):
                    try:
                        with os.scandir(path) as it:
                            n = sum(1 for _ in it)
                        text = fc.item_count_text(n)
                    except OSError as e:
                        text = e.strerror or "Not available"
                    GLib.idle_add(lambda: (gen == self._gen and count.set_label(text), False)[1])
                threading.Thread(target=worker, daemon=True).start()
            elif count is not None:
                count.set_label("")
        self._row("Date modified", _date_text(item.mtime))
        created = item.info.get_creation_date_time() if item.info.has_attribute(
            "time::created") else None
        self._row("Date created", _date_text(created.to_unix()) if created else None)
        if item.orig_path:
            self._row("Original location", os.path.dirname(item.orig_path))
            self._row("Date deleted", _date_text(item.deleted))
        elif item.path:
            self._row("Location", os.path.dirname(item.path))


# ---------------------------------------------------------------------- properties

class PropertiesDialog(Adw.Dialog):
    """Explorer-style General properties for one or more files."""

    def __init__(self, win, gfiles):
        super().__init__(content_width=460)
        self.win = win
        self.gfiles = list(gfiles)
        self.stop = threading.Event()
        self.connect("closed", lambda *_: self.stop.set())
        tv = Adw.ToolbarView()
        tv.add_top_bar(Adw.HeaderBar())
        self.body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0, margin_start=22,
                            margin_end=22, margin_top=6, margin_bottom=22)
        sw = Gtk.ScrolledWindow(child=self.body, propagate_natural_height=True,
                                max_content_height=640)
        sw.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        tv.set_content(sw)
        self.set_child(tv)
        self.grid = None
        self.values = {}
        self._presented = False
        if len(self.gfiles) == 1:
            self.set_title("%s Properties" % (self._name(self.gfiles[0])))
            # Present once the info is in, so the dialog opens at its full size;
            # a slow (network) file still gets the dialog after a short wait.
            self.gfiles[0].query_info_async(INFO_ATTRS, Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS,
                                            GLib.PRIORITY_DEFAULT, None, self._got_info)
            GLib.timeout_add(400, self._present)
        else:
            self.set_title("Properties")
            self._build_multi()
            self._present()

    def _present(self):
        if not self._presented and self.win is not None:
            self._presented = True
            self.present(self.win)
        return False

    @staticmethod
    def _name(g):
        if g.get_uri_scheme() == "trash" and g.get_uri().rstrip("/") == "trash:":
            return "Trash"
        try:
            info = g.query_info("standard::display-name", Gio.FileQueryInfoFlags.NONE, None)
            return info.get_display_name()
        except GLib.Error:
            return g.get_basename() or g.get_uri()

    def _header(self, icon, name):
        box = Gtk.Box(spacing=14, margin_bottom=14)
        box.append(_image(icon, 48))
        box.append(_label(name, "hbf-prop-name", wrap=True, hexpand=True,
                          valign=Gtk.Align.CENTER))
        self.body.append(box)

    def _group(self):
        # one grid for every group, so the value column lines up like Explorer
        if self.grid is None:
            self.grid = Gtk.Grid(column_spacing=18, row_spacing=7, margin_bottom=6)
            self.grid.n = 0
            self.body.append(self.grid)
        self.grid.attach(Gtk.Separator(margin_top=4, margin_bottom=4, css_classes=["hbf-sep"]),
                         0, self.grid.n, 2, 1)
        self.grid.n += 1

    def _kv(self, key, value, name=None):
        if value in (None, "") and name is None:
            return None
        self.grid.attach(_label(key + ":", "hb-dim", valign=Gtk.Align.START), 0, self.grid.n, 1, 1)
        v = _label(str(value or ""), wrap=True, selectable=True, hexpand=True)
        v.set_focusable(False)   # still mouse-selectable, but not auto-focused and highlighted
        self.grid.attach(v, 1, self.grid.n, 1, 1)
        self.grid.n += 1
        if name:
            self.values[name] = v
        return v

    def _got_info(self, g, res):
        try:
            self._fill(g, res)
        finally:
            self._present()

    def _fill(self, g, res):
        try:
            info = g.query_info_finish(res)
        except GLib.Error as e:
            self._header("dialog-error-symbolic", g.get_basename() or g.get_uri())
            self._group()
            self._kv("Error", e.message)
            return
        ftype = info.get_file_type()
        is_dir = ftype in (Gio.FileType.DIRECTORY, Gio.FileType.MOUNTABLE)
        ct = info.get_content_type() or ""
        name = info.get_display_name() or g.get_basename()
        if g.get_uri().rstrip("/") == "trash:":
            name = "Trash"
        self._header(info.get_icon() or "text-x-generic", name)
        self._group()
        desc = "File folder" if is_dir else (Gio.content_type_get_description(ct) if ct else "File")
        if info.get_is_symlink():
            desc += " (link to %s)" % (info.get_symlink_target() or "?")
        self._kv("Type", desc)
        if not is_dir and ct:
            app = Gio.AppInfo.get_default_for_type(ct, False)
            self._kv("Opens with", app.get_display_name() if app else "No app set")
        path = g.get_path()
        parent = g.get_parent()
        orig = info.get_attribute_byte_string("trash::orig-path")
        if orig:
            self._kv("Original location", os.path.dirname(orig))
        elif parent is not None:
            self._kv("Location", parent.get_path() or fc.uri_to_label(parent.get_uri()))
        self._group()
        if is_dir and path and not info.get_is_symlink():
            self._kv("Size", "Calculating…", name="size")
            self._kv("Contains", "Calculating…", name="contains")
            self._count([path])
        elif g.get_uri().rstrip("/") == "trash:":
            self._kv("Contains", "%s" % fc.item_count_text(
                info.get_attribute_uint32("trash::item-count")))
        else:
            size = info.get_size()
            self._kv("Size", "%s (%s bytes)" % (human_size(size), "{:,}".format(size)))
        if path and info.has_attribute("filesystem::free") and is_dir:
            free = info.get_attribute_uint64("filesystem::free")
            total = info.get_attribute_uint64("filesystem::size")
            if total:
                self._kv("Free space", "%s free of %s" % (human_size(free), human_size(total)))
        self._group()
        for key, attr in (("Created", "time::created"), ("Modified", "time::modified"),
                          ("Accessed", "time::access")):
            if info.has_attribute(attr):
                self._kv(key, _date_text(info.get_attribute_uint64(attr)))
        if info.has_attribute("trash::deletion-date"):
            dd = info.get_deletion_date()
            self._kv("Deleted", _date_text(dd.to_unix()) if dd else None)
        if info.has_attribute("unix::mode"):
            self._group()
            mode = info.get_attribute_uint32("unix::mode")
            self._kv("Permissions", "%s  (%o)" % (fc.permission_string(mode), mode & 0o7777))
            owner = info.get_attribute_string("owner::user")
            group = info.get_attribute_string("owner::group")
            if owner:
                self._kv("Owner", "%s (group %s)" % (owner, group) if group else owner)
            if info.has_attribute("access::can-write"):
                self._kv("Attributes", "Read-only" if not info.get_attribute_boolean(
                    "access::can-write") else "Read and write")

    def _build_multi(self):
        paths = [g.get_path() for g in self.gfiles]
        self._header("edit-select-all-symbolic", "%d items" % len(self.gfiles))
        self._group()
        parents = {os.path.dirname(p) for p in paths if p}
        if len(parents) == 1:
            self._kv("Location", parents.pop())
        else:
            self._kv("Location", "Various folders")
        self._group()
        self._kv("Size", "Calculating…", name="size")
        self._kv("Contains", "Calculating…", name="contains")
        self._count([p for p in paths if p])

    def _count(self, paths):
        stop = self.stop

        def work():
            size = files = folders = 0
            for p in paths:
                if stop.is_set():
                    break
                s, f, d = fc.tree_size(p, stop)
                size, files, folders = size + s, files + f, folders + d
            return size, files, folders

        def done(res):
            if stop.is_set() or isinstance(res, Exception):
                return
            size, files, folders = res
            lab = self.values.get("size")
            if lab is not None:
                lab.set_label("%s (%s bytes)" % (human_size(size), "{:,}".format(size)))
            lab = self.values.get("contains")
            if lab is not None:
                lab.set_label("%d %s, %d %s" % (files, "file" if files == 1 else "files", folders,
                                                "folder" if folders == 1 else "folders"))
        fc.run_in_thread(work, done)


# ---------------------------------------------------------------------- open with

class AppChooser(Adw.Dialog):
    """Pick any installed app to open a file with."""

    def __init__(self, win, gfile, content_type):
        super().__init__(title="Open with", content_width=420, content_height=520)
        self.win, self.gfile, self.ct = win, gfile, content_type
        tv = Adw.ToolbarView()
        tv.add_top_bar(Adw.HeaderBar())
        body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, margin_start=16,
                       margin_end=16, margin_top=4, margin_bottom=16)
        desc = Gio.content_type_get_description(content_type) if content_type else "file"
        body.append(_label("How do you want to open “%s”?" % (gfile.get_basename() or ""),
                           wrap=True))
        self.search = Gtk.SearchEntry(placeholder_text="Search apps", css_classes=["hb-search"])
        self.search.connect("search-changed", lambda *_: self.list.invalidate_filter())
        body.append(self.search)
        self.list = Gtk.ListBox(css_classes=["hbf-apps"], activate_on_single_click=False)
        self.list.set_filter_func(self._filter)
        self.list.connect("row-activated", lambda *_: self._open())
        sw = Gtk.ScrolledWindow(child=self.list, vexpand=True, css_classes=["hbf-apps-scroll"])
        body.append(sw)
        self.always = Gtk.CheckButton(label="Always use this app for %s" % desc)
        body.append(self.always)
        btns = Gtk.Box(spacing=8, halign=Gtk.Align.END)
        cancel = Gtk.Button(label="Cancel", css_classes=["hb-plain"])
        cancel.connect("clicked", lambda *_: self.close())
        ok = Gtk.Button(label="Open", css_classes=["hb-accent"])
        ok.connect("clicked", lambda *_: self._open())
        btns.append(cancel)
        btns.append(ok)
        body.append(btns)
        tv.set_content(body)
        self.set_child(tv)
        rec = Gio.AppInfo.get_recommended_for_type(content_type) if content_type else []
        rec_ids = {a.get_id() for a in rec}
        others = sorted((a for a in Gio.AppInfo.get_all()
                         if a.should_show() and a.get_id() not in rec_ids),
                        key=lambda a: (a.get_display_name() or "").casefold())
        for a in list(rec) + others:
            row = Gtk.ListBoxRow()
            box = Gtk.Box(spacing=10, margin_top=6, margin_bottom=6, margin_start=6)
            box.append(_image(a.get_icon() or "application-x-executable", 24))
            box.append(_label(a.get_display_name() or a.get_name() or a.get_id(), hexpand=True))
            if a.get_id() in rec_ids:
                box.append(_label("Suggested", "hb-dim", "hb-small"))
            row.set_child(box)
            row.app = a
            self.list.append(row)
        first = self.list.get_row_at_index(0)
        if first is not None:
            self.list.select_row(first)

    def _filter(self, row):
        q = self.search.get_text().strip().casefold()
        if not q:
            return True
        a = row.app
        return q in (a.get_display_name() or "").casefold() or q in (a.get_id() or "").casefold()

    def _open(self):
        row = self.list.get_selected_row()
        if row is None:
            return
        app = row.app
        if self.always.get_active() and self.ct:
            self.win.set_default_app(app, self.ct)
        self.close()
        self.win.launch(self.gfile.get_uri(), app)


# ---------------------------------------------------------------------- rename

class RenamePopover(Gtk.Popover):
    """Inline rename box (F2). on_done(new_name) is called on Enter."""

    def __init__(self, parent, name, is_dir, on_done):
        super().__init__(has_arrow=False, css_classes=["hbf-rename"], autohide=True)
        self.set_parent(parent)
        self.entry = Gtk.Entry(text=name, width_chars=max(18, min(48, len(name) + 4)),
                               css_classes=["hb-search"])
        self.set_child(self.entry)
        self.on_done = on_done
        self._sel = fc.rename_selection_range(name, is_dir)
        self.entry.connect("activate", self._activate)
        key = Gtk.EventControllerKey()
        key.connect("key-pressed", self._key)
        self.entry.add_controller(key)
        self.connect("closed", lambda *_: GLib.idle_add(self._cleanup))
        self.connect("map", lambda *_: GLib.idle_add(self._select))

    def _select(self):
        self.entry.grab_focus()
        start, end = self._sel
        self.entry.select_region(start, end)
        return False

    def _key(self, _c, keyval, _code, _state):
        if keyval == Gdk.KEY_Escape:
            self.popdown()
            return True
        return False

    def _activate(self, *_):
        name = self.entry.get_text()
        if self.on_done(name) is not False:
            self.popdown()

    def _cleanup(self):
        if self.get_parent() is not None:
            self.unparent()
        return False
