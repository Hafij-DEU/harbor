"""Harbor Files: the folder view (details table + icon grid).

One FolderView shows a folder, the trash or search results. It loads
asynchronously in batches, follows a Gio.FileMonitor, keeps folders first
and shares one selection between the details and the icon views.
"""

import os
import threading
import time
from concurrent.futures import ThreadPoolExecutor

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("GdkPixbuf", "2.0")
gi.require_version("Graphene", "1.0")
from gi.repository import Gdk, GdkPixbuf, Gio, GLib, GObject, Graphene, Gtk, Pango  # noqa: E402

from . import files_core as fc  # noqa: E402
from . import files_icons  # noqa: E402
from .common import human_size  # noqa: E402

ATTRS = ",".join((
    "standard::name", "standard::display-name", "standard::type", "standard::size",
    "standard::icon", "standard::symbolic-icon", "standard::content-type",
    "standard::fast-content-type", "standard::is-hidden", "standard::is-backup",
    "standard::is-symlink", "standard::target-uri", "time::modified", "time::created",
    "thumbnail::path", "thumbnail::is-valid", "trash::orig-path", "trash::deletion-date",
    "access::can-rename", "access::can-trash", "access::can-delete", "access::can-write",
) + files_icons.METADATA_ATTRS)

# Settings > Preview (set by the window; the view has no settings access)
OPTIONS = {"image_thumbnails": True, "doc_thumbnails": False, "remote_thumbnails": False}

ICON_SIZES = {"large": (96, 124), "medium": (56, 96)}  # icon px, tile width

_THUMBS = ThreadPoolExecutor(max_workers=2, thread_name_prefix="hb-thumb")
THUMB_MAX_BYTES = 25 * 1024 * 1024


_TYPE_DESC = {}


def type_description(ct):
    """Gio.content_type_get_description, cached (it reads shared-mime-info each call)."""
    d = _TYPE_DESC.get(ct)
    if d is None:
        d = _TYPE_DESC[ct] = Gio.content_type_get_description(ct) or ct
    return d


class FileItem(GObject.Object):
    """One row: a Gio.File plus the bits of its FileInfo the views need."""

    def __init__(self, gfile, info, extra_location=None):
        super().__init__()
        self.gfile = gfile
        self.info = info
        self.path = gfile.get_path()
        self.uri = gfile.get_uri()
        self.name = info.get_display_name() or info.get_name() or ""
        ftype = info.get_file_type()
        self.is_dir = ftype in (Gio.FileType.DIRECTORY, Gio.FileType.MOUNTABLE)
        self.size = info.get_size() if not self.is_dir else 0
        dt = info.get_modification_date_time()
        self.mtime = dt.to_unix() if dt else 0
        ct = info.get_content_type() or info.get_attribute_string("standard::fast-content-type") or ""
        self.content_type = ct
        if self.is_dir:
            self.type_desc = "File folder"
        elif ct:
            self.type_desc = type_description(ct)
        else:
            self.type_desc = "File"
        self.icon = files_icons.item_gicon(info, self.path, self.is_dir)
        # gvfs (sftp, ftp, smb...) has no local path or a FUSE one
        self.remote = not gfile.is_native() or (self.path or "").startswith("/run/user/")
        self.hidden = bool(
            (info.has_attribute("standard::is-hidden") and info.get_is_hidden())
            or (info.has_attribute("standard::is-backup") and info.get_is_backup()))
        self.thumb_path = info.get_attribute_byte_string("thumbnail::path")
        self.orig_path = info.get_attribute_byte_string("trash::orig-path")
        dd = info.get_deletion_date() if info.has_attribute("trash::deletion-date") else None
        self.deleted = dd.to_unix() if dd else 0
        self.location = extra_location
        self.texture = None
        self.thumb_state = 0  # 0 untried, 1 loading, 2 done
        self.cells = []        # bound cell widgets (for refreshes / rename popover)
        self._keys = {}        # sort keys per field, built once (sorting calls back a lot)

    def sort_key(self, field):
        k = self._keys.get(field)
        if k is None:
            if field == "name":
                k = fc.natural_key(self.name)
            else:
                k = fc.sort_key(field, self.name, self.is_dir, self.mtime, self.type_desc, self.size)
            self._keys[field] = k
        return k

    @property
    def is_image(self):
        return self.content_type.startswith("image/") and not self.is_dir

    def can(self, what):
        attr = "access::can-" + what
        if not self.info.has_attribute(attr):
            return True
        return self.info.get_attribute_boolean(attr)


def _load_thumb(item, size):
    """Worker: a small texture for an image (existing thumbnail first)."""
    src = item.thumb_path if item.thumb_path and os.path.exists(item.thumb_path) else item.path
    if not src:
        return None
    if src == item.path and item.size > THUMB_MAX_BYTES:
        return None
    try:
        pb = GdkPixbuf.Pixbuf.new_from_file_at_scale(src, size, size, True)
        pb = pb.apply_embedded_orientation() or pb
    except GLib.Error:
        return None
    fmt = Gdk.MemoryFormat.R8G8B8A8 if pb.get_has_alpha() else Gdk.MemoryFormat.R8G8B8
    return Gdk.MemoryTexture.new(pb.get_width(), pb.get_height(), fmt, pb.read_pixel_bytes(),
                                 pb.get_rowstride())


def _load_doc_thumb(item, size):
    """Worker: page 1 of an office document (Settings > Document thumbnails)."""
    from . import files_office
    if not item.path or item.size > 50 * 1024 * 1024:
        return None
    png = files_office.thumbnail(item.path, item.mtime, item.size)
    if not png:
        return None
    try:
        pb = GdkPixbuf.Pixbuf.new_from_file_at_scale(png, size, size, True)
    except GLib.Error:
        return None
    fmt = Gdk.MemoryFormat.R8G8B8A8 if pb.get_has_alpha() else Gdk.MemoryFormat.R8G8B8
    return Gdk.MemoryTexture.new(pb.get_width(), pb.get_height(), fmt, pb.read_pixel_bytes(),
                                 pb.get_rowstride())


def thumb_loader(item, icon_px, options=None):
    """Which worker makes this item's grid thumbnail, or None."""
    o = OPTIONS if options is None else options
    if item.is_dir or not item.path:
        return None
    if item.remote and not o.get("remote_thumbnails"):
        return None
    if item.is_image:
        return _load_thumb if o.get("image_thumbnails", True) else None
    if o.get("doc_thumbnails") and icon_px >= 96 and fc.office_kind(item.name):
        return _load_doc_thumb
    return None


def find_item_widget(widget, root):
    """Walk up from a picked widget to the cell that carries _hb_item."""
    w = widget
    while w is not None and w is not root:
        if getattr(w, "_hb_item", None) is not None:
            return w._hb_item
        if w.get_css_name() in ("row", "child"):
            stack = [w.get_first_child()]
            while stack:
                c = stack.pop()
                while c is not None:
                    if getattr(c, "_hb_item", None) is not None:
                        return c._hb_item
                    stack.append(c.get_first_child())
                    c = c.get_next_sibling()
            return None
        w = w.get_parent()
    return None


def widget_point(src, target, x, y):
    """(x, y) of point (x, y) of src in target's coordinates.

    Uses Gtk.Widget.compute_point (GTK >= 4.12). translate_coordinates is
    deprecated and its return shape differs between PyGObject versions, so it
    is only a fallback and both shapes are accepted. Never raises: falls back
    to the input point.
    """
    try:
        res = src.compute_point(target, Graphene.Point().init(x, y))
        ok, pt = res if isinstance(res, tuple) else (res is not None, res)
        if ok and pt is not None:
            return pt.x, pt.y
    except (AttributeError, TypeError):
        pass
    try:
        res = src.translate_coordinates(target, x, y)
        if isinstance(res, tuple):
            if len(res) == 3 and res[0]:
                return res[1], res[2]
            if len(res) == 2:
                return res[0], res[1]
    except (AttributeError, TypeError):
        pass
    return x, y


class FolderView(Gtk.Box):
    """Details/icon view of one location."""

    __gsignals__ = {
        "item-activated": (GObject.SignalFlags.RUN_FIRST, None, (object,)),
        "selection-changed": (GObject.SignalFlags.RUN_FIRST, None, ()),
        "context-menu": (GObject.SignalFlags.RUN_FIRST, None, (float, float, object)),
        "middle-click": (GObject.SignalFlags.RUN_FIRST, None, (object,)),
        "contents-changed": (GObject.SignalFlags.RUN_FIRST, None, ()),
        "drop-files": (GObject.SignalFlags.RUN_FIRST, None, (object, object, bool)),
        "needs-mount": (GObject.SignalFlags.RUN_FIRST, None, ()),
    }

    def __init__(self, show_hidden=False, view="details", icon_size="large"):
        super().__init__(orientation=Gtk.Orientation.VERTICAL)
        self.add_css_class("hbf-view")
        self.mode = "folder"        # folder | trash | search
        self.gfile = None
        self.show_hidden = show_hidden
        self.view = view
        self.icon_size = icon_size
        self.loading = False
        self.error = None
        self.by_uri = {}
        self.pending_select = set()
        self._cancel = None
        self._monitor = None
        self._gen = 0
        self._search_stop = None
        self.sort_field = "name"
        self.descending = False
        self.show_extensions = True

        self.store = Gio.ListStore(item_type=FileItem)
        self.filter = Gtk.CustomFilter.new(self._filter_func)
        self.filter_model = Gtk.FilterListModel(model=self.store, filter=self.filter)
        self.multi = Gtk.MultiSorter()
        self.multi.append(Gtk.CustomSorter.new(self._folders_first))
        self.sort_model = Gtk.SortListModel(model=self.filter_model, sorter=self.multi)
        self.selection = Gtk.MultiSelection(model=self.sort_model)
        self.selection.connect("selection-changed", lambda *_: self.emit("selection-changed"))
        self.sort_model.connect("items-changed", self._on_items_changed)

        self._build_details()
        self._build_grid()

        self.stack = Gtk.Stack(vexpand=True, hhomogeneous=False, vhomogeneous=False)
        self.cv_scroll = Gtk.ScrolledWindow(child=self.cv, vexpand=True)
        self.grid_scroll = Gtk.ScrolledWindow(child=self.grid, vexpand=True)
        self.grid_scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.stack.add_named(self.cv_scroll, "details")
        self.stack.add_named(self.grid_scroll, "icons")
        self.empty = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8,
                             halign=Gtk.Align.CENTER, valign=Gtk.Align.START, margin_top=70,
                             can_target=False)
        self.empty_label = Gtk.Label(css_classes=["hb-dim"], wrap=True, justify=Gtk.Justification.CENTER)
        self.empty.append(self.empty_label)
        overlay = Gtk.Overlay(child=self.stack, vexpand=True)
        overlay.add_overlay(self.empty)
        self.append(overlay)

        for w in (self.cv, self.grid):
            click = Gtk.GestureClick(button=0)
            click.connect("pressed", self._on_click, w)
            w.add_controller(click)
            drop = Gtk.DropTarget.new(Gdk.FileList, Gdk.DragAction.COPY | Gdk.DragAction.MOVE)
            drop.connect("drop", self._on_drop)
            drop.connect("enter", self._on_drop_motion)
            drop.connect("motion", self._on_drop_motion)
            w.add_controller(drop)
        self.set_view(view)
        self.set_sort("name", False)

    # ------------------------------------------------------------ building

    def _build_details(self):
        self.cv = Gtk.ColumnView(model=self.selection, css_classes=["hb-table", "hbf-table"],
                                 enable_rubberband=True, show_column_separators=False,
                                 reorderable=False, tab_behavior=Gtk.ListTabBehavior.ITEM)
        self.cv.connect("activate", self._on_activate)
        self.columns = {}

        def col(key, title, setup, bind, width=None, expand=False, field=None):
            f = Gtk.SignalListItemFactory()
            f.connect("setup", setup)
            f.connect("bind", bind)
            f.connect("unbind", self._unbind)
            c = Gtk.ColumnViewColumn(title=title, factory=f, expand=expand, resizable=True)
            if width:
                c.set_fixed_width(width)
            if field:
                c.set_sorter(Gtk.CustomSorter.new(self._field_sorter(field)))
            self.cv.append_column(c)
            self.columns[key] = c
            return c

        def text_setup(xalign=0.0, dim=True):
            def setup(_f, cell):
                lab = Gtk.Label(xalign=xalign, ellipsize=Pango.EllipsizeMode.END,
                                css_classes=["hbf-cell"] + (["hb-dim"] if dim else []))
                cell.set_child(lab)
            return setup

        def text_bind(getter):
            def bind(_f, cell):
                item = cell.get_item()
                lab = cell.get_child()
                lab._hb_item = item
                lab.set_label(getter(item))
            return bind

        col("name", "Name", self._name_setup, self._name_bind, expand=True, field="name")
        col("location", "Location", text_setup(), text_bind(lambda i: i.location or ""), width=260)
        col("orig", "Original location", text_setup(),
            text_bind(lambda i: os.path.dirname(i.orig_path) if i.orig_path else ""), width=260)
        col("deleted", "Date deleted", text_setup(),
            text_bind(lambda i: fc.format_time(i.deleted)), width=150)
        col("modified", "Date modified", text_setup(),
            text_bind(lambda i: fc.format_time(i.mtime)), width=150, field="modified")
        col("type", "Type", text_setup(), text_bind(lambda i: i.type_desc), width=150, field="type")
        col("size", "Size", text_setup(1.0),
            text_bind(lambda i: "" if i.is_dir else human_size(i.size)), width=96, field="size")
        self._set_column_mode("folder")

    def _name_setup(self, _f, cell):
        box = Gtk.Box(spacing=10, css_classes=["hbf-name"])
        img = Gtk.Image(pixel_size=20)
        lab = Gtk.Label(xalign=0, ellipsize=Pango.EllipsizeMode.END, hexpand=True)
        box.append(img)
        box.append(lab)
        box.img, box.lab = img, lab
        cell.set_child(box)
        self._add_drag_source(box)

    def _name_bind(self, _f, cell):
        item = cell.get_item()
        box = cell.get_child()
        box._hb_item = item
        item.cells.append(box)
        if item.icon:
            box.img.set_from_gicon(item.icon)
        else:
            box.img.set_from_icon_name(files_icons.themed("text-x-generic"))
        box.lab.set_label(self.label_for(item))
        box.set_opacity(0.55 if item.hidden else 1.0)

    def _unbind(self, _f, cell):
        child = cell.get_child()
        item = getattr(child, "_hb_item", None)
        if item is not None and child in item.cells:
            item.cells.remove(child)
        child._hb_item = None

    def _build_grid(self):
        f = Gtk.SignalListItemFactory()
        f.connect("setup", self._grid_setup)
        f.connect("bind", self._grid_bind)
        f.connect("unbind", self._unbind)
        self.grid = Gtk.GridView(model=self.selection, factory=f, max_columns=40, min_columns=1,
                                 enable_rubberband=True, css_classes=["hbf-grid"],
                                 tab_behavior=Gtk.ListTabBehavior.ITEM)
        self.grid.connect("activate", self._on_activate)

    def _grid_setup(self, _f, cell):
        icon_px, tile_w = ICON_SIZES.get(self.icon_size, ICON_SIZES["large"])
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4, css_classes=["hbf-tile"],
                      width_request=tile_w, halign=Gtk.Align.CENTER)
        img = Gtk.Image(pixel_size=icon_px, halign=Gtk.Align.CENTER, height_request=icon_px)
        lab = Gtk.Label(justify=Gtk.Justification.CENTER, wrap=True,
                        wrap_mode=Pango.WrapMode.WORD_CHAR, lines=2,
                        ellipsize=Pango.EllipsizeMode.END, max_width_chars=1,
                        width_request=tile_w - 12, halign=Gtk.Align.CENTER, css_classes=["hbf-tile-label"])
        box.append(img)
        box.append(lab)
        box.img, box.lab, box.icon_px = img, lab, icon_px
        cell.set_child(box)
        self._add_drag_source(box)

    def _grid_bind(self, _f, cell):
        item = cell.get_item()
        box = cell.get_child()
        box._hb_item = item
        item.cells.append(box)
        box.lab.set_label(self.label_for(item))
        box.set_opacity(0.55 if item.hidden else 1.0)
        self._set_grid_icon(box, item)
        loader = thumb_loader(item, box.icon_px) if item.thumb_state == 0 else None
        if loader is not None:
            item.thumb_state = 1
            gen = (self._gen, getattr(self, "_tgen", 0))
            size = 128
            fut = _THUMBS.submit(loader, item, size)
            fut.add_done_callback(lambda fu: GLib.idle_add(self._thumb_done, item, fu, gen))

    def _set_grid_icon(self, box, item):
        if item.texture is not None:
            box.img.set_from_paintable(item.texture)
        elif item.icon:
            box.img.set_from_gicon(item.icon)
        else:
            box.img.set_from_icon_name(files_icons.themed("text-x-generic"))

    def _thumb_done(self, item, fut, gen):
        item.thumb_state = 2
        try:
            tex = fut.result()
        except Exception:
            tex = None
        if tex is None or gen != (self._gen, getattr(self, "_tgen", 0)):
            return False
        item.texture = tex
        for box in list(item.cells):
            if hasattr(box, "icon_px"):
                self._set_grid_icon(box, item)
        return False

    def _add_drag_source(self, widget):
        src = Gtk.DragSource(actions=Gdk.DragAction.COPY | Gdk.DragAction.MOVE)
        src.connect("prepare", self._drag_prepare, widget)
        src.connect("drag-begin", self._drag_begin, widget)
        widget.add_controller(src)

    def _drag_prepare(self, _src, _x, _y, widget):
        item = getattr(widget, "_hb_item", None)
        if item is None:
            return None
        if item not in self.selected_items():
            self.select_item(item)
        files = [i.gfile for i in self.selected_items()]
        if not files:
            return None
        return Gdk.ContentProvider.new_for_value(Gdk.FileList.new_from_list(files))

    def _drag_begin(self, src, _drag, widget):
        item = getattr(widget, "_hb_item", None)
        if item is not None and item.icon:
            theme = Gtk.IconTheme.get_for_display(self.get_display())
            icon = theme.lookup_by_gicon(item.icon, 48, 1, Gtk.TextDirection.NONE, 0)
            src.set_icon(icon, 16, 16)

    # ------------------------------------------------------------ drop

    def _on_drop_motion(self, target, _x, _y):
        drop = target.get_current_drop()
        return self._pick_action(drop, self.gfile)

    def _pick_action(self, drop, dest):
        if self.mode != "folder" or dest is None:
            return 0
        return pick_drop_action(drop, dest)

    def _on_drop(self, target, value, x, y):
        if self.mode != "folder" or self.gfile is None:
            return False
        files = value.get_files() if hasattr(value, "get_files") else []
        drop = target.get_current_drop()
        dest = self.gfile
        hit = find_item_widget(target.get_widget().pick(x, y, Gtk.PickFlags.DEFAULT), target.get_widget())
        if hit is not None and hit.is_dir and hit.path and hit.gfile not in files:
            dest = hit.gfile
        action = pick_drop_action(drop, dest, files)
        self.emit("drop-files", files, dest, action == Gdk.DragAction.MOVE)
        return True

    # ------------------------------------------------------------ filter / sort

    def _filter_func(self, item, *_):
        return self.show_hidden or not item.hidden or self.mode == "trash"

    def _folders_first(self, a, b, *_):
        return (b.is_dir - a.is_dir) if a.is_dir != b.is_dir else 0

    def _field_sorter(self, field):
        def cmp(a, b, *_):
            ka, kb = a.sort_key(field), b.sort_key(field)
            return (ka > kb) - (ka < kb)
        return cmp

    def set_sort(self, field, descending):
        self.sort_field, self.descending = field, descending
        colkey = {"name": "name", "modified": "modified", "type": "type", "size": "size"}[field]
        direction = Gtk.SortType.DESCENDING if descending else Gtk.SortType.ASCENDING
        if not getattr(self, "_cv_sorter_added", False):
            self.multi.append(self.cv.get_sorter())
            self._cv_sorter_added = True
        self.cv.sort_by_column(self.columns[colkey], direction)

    def current_sort(self):
        """(field, descending) as chosen by the column headers."""
        s = self.cv.get_sorter()
        col = s.get_primary_sort_column() if s else None
        field = "name"
        for k, c in self.columns.items():
            if c == col:
                field = k
        desc = s.get_primary_sort_order() == Gtk.SortType.DESCENDING if s else False
        return field, desc

    def label_for(self, item):
        if self.show_extensions or item.is_dir:
            return item.name
        stem, _ext = fc.split_ext(item.name)
        return stem or item.name

    def set_show_extensions(self, value):
        if value == self.show_extensions:
            return
        self.show_extensions = value
        for item in self.items():
            for box in list(item.cells):
                if hasattr(box, "lab"):
                    box.lab.set_label(self.label_for(item))

    def refresh_icons(self):
        """Icons setting changed: re-pick every item's icon and update shown cells."""
        for item in self.items_all():
            item.icon = files_icons.item_gicon(item.info, item.path, item.is_dir)
            for box in list(item.cells):
                if not hasattr(box, "img"):
                    continue
                if item.texture is not None and box.img.get_pixel_size() > 32:
                    continue
                box.img.set_from_gicon(item.icon)

    def refresh_thumbs(self):
        """Thumbnail settings changed: drop the pictures and bind again."""
        for item in self.items_all():
            item.texture = None
            item.thumb_state = 0
        self._tgen = getattr(self, "_tgen", 0) + 1
        f = self.grid.get_factory()
        self.grid.set_factory(None)
        self.grid.set_factory(f)

    def set_single_click(self, value):
        """Settings > Open items with: single click (like a web page)."""
        self.grid.set_single_click_activate(bool(value))
        self.cv.set_single_click_activate(bool(value))

    def items_all(self):
        return [self.store.get_item(i) for i in range(self.store.get_n_items())]

    def set_show_hidden(self, value):
        self.show_hidden = value
        self.filter.changed(Gtk.FilterChange.DIFFERENT)

    def set_view(self, view, icon_size=None):
        self.view = view
        if icon_size and icon_size != self.icon_size:
            self.icon_size = icon_size
            f = self.grid.get_factory()
            self.grid.set_factory(None)
            self.grid.set_factory(f)
        self.stack.set_visible_child_name("details" if view == "details" else "icons")

    def _set_column_mode(self, mode):
        vis = {"folder": ("name", "modified", "type", "size"),
               "search": ("name", "location", "modified", "size"),
               "trash": ("name", "orig", "deleted", "size")}[mode]
        for k, c in self.columns.items():
            c.set_visible(k in vis)

    # ------------------------------------------------------------ loading

    def _stop(self):
        self._gen += 1
        if self._cancel:
            self._cancel.cancel()
            self._cancel = None
        if self._search_stop:
            self._search_stop.set()
            self._search_stop = None
        if self._monitor:
            self._monitor.cancel()
            self._monitor = None
        self.loading = False

    def stop_search(self):
        if self._search_stop:
            self._search_stop.set()
            self._search_stop = None
            self.loading = False
            self._update_empty()
            self.emit("contents-changed")

    def _reset(self, mode):
        self._stop()
        self.mode = mode
        self.error = None
        self.by_uri = {}
        self.store.remove_all()
        self._set_column_mode(mode)
        # search results keep arriving: sort them a chunk per frame instead of
        # re-sorting the whole list in one go every time a batch lands
        self.sort_model.set_incremental(mode == "search")
        self.filter.changed(Gtk.FilterChange.DIFFERENT)

    def load(self, gfile, mode="folder"):
        """Show a folder (mode folder) or the trash (gfile trash:///)."""
        self._reset(mode)
        self.gfile = gfile
        self.loading = True
        self._update_empty()
        self._cancel = Gio.Cancellable()
        gen = self._gen
        gfile.enumerate_children_async(ATTRS, Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_DEFAULT,
                                       self._cancel, self._on_enum, gen)
        try:
            self._monitor = gfile.monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, None)
            self._monitor.set_rate_limit(300)
            self._monitor.connect("changed", self._on_monitor, gen)
        except GLib.Error:
            self._monitor = None

    def reload(self):
        if self.mode in ("folder", "trash") and self.gfile is not None:
            sel = {i.uri for i in self.selected_items()}
            self.pending_select |= sel
            self.load(self.gfile, self.mode)

    def _on_enum(self, gfile, res, gen):
        if gen != self._gen:
            return
        try:
            enum = gfile.enumerate_children_finish(res)
        except GLib.Error as e:
            if e.matches(Gio.io_error_quark(), Gio.IOErrorEnum.NOT_MOUNTED):
                self.loading = False
                self.emit("needs-mount")
                return
            if not e.matches(Gio.io_error_quark(), Gio.IOErrorEnum.CANCELLED):
                self.error = e.message
                self.loading = False
                self._update_empty()
                self.emit("contents-changed")
            return
        enum.next_files_async(200, GLib.PRIORITY_DEFAULT, self._cancel, self._on_batch, gen)

    def _on_batch(self, enum, res, gen):
        if gen != self._gen:
            return
        try:
            infos = enum.next_files_finish(res)
        except GLib.Error as e:
            if not e.matches(Gio.io_error_quark(), Gio.IOErrorEnum.CANCELLED):
                self.error = e.message
            infos = []
        if infos:
            items = []
            for info in infos:
                child = enum.get_child(info)
                it = FileItem(child, info)
                if it.uri in self.by_uri:
                    continue
                self.by_uri[it.uri] = it
                items.append(it)
            self.store.splice(self.store.get_n_items(), 0, items)
            self._select_pending(items)
            enum.next_files_async(200, GLib.PRIORITY_DEFAULT, self._cancel, self._on_batch, gen)
        else:
            enum.close_async(GLib.PRIORITY_DEFAULT, None, None, None)
            self.loading = False
            self._update_empty()
            self.emit("contents-changed")

    def _on_monitor(self, _mon, gfile, other, event, gen):
        if gen != self._gen:
            return
        # Events about the folder itself (the trash sends item-count changes)
        # or about deeper paths are not rows of this listing.
        if self.gfile is not None and not self._is_child(gfile):
            if event == Gio.FileMonitorEvent.RENAMED and other is not None \
                    and self._is_child(other):
                self._query_add(other)
            return
        E = Gio.FileMonitorEvent
        if event in (E.CREATED, E.MOVED_IN):
            self._query_add(gfile)
        elif event in (E.DELETED, E.MOVED_OUT):
            self._remove(gfile)
        elif event == E.RENAMED:
            self._remove(gfile)
            if other is not None:
                self._query_add(other)
        elif event in (E.CHANGES_DONE_HINT, E.ATTRIBUTE_CHANGED):
            self._query_add(gfile)

    def _is_child(self, gfile):
        parent = gfile.get_parent()
        return parent is not None and parent.equal(self.gfile)

    def _query_add(self, gfile):
        gen = self._gen

        def done(f, res):
            if gen != self._gen:
                return
            try:
                info = f.query_info_finish(res)
            except GLib.Error:
                return
            self._add_or_update(f, info)
        gfile.query_info_async(ATTRS, Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_DEFAULT, None, done)

    def _add_or_update(self, gfile, info):
        item = FileItem(gfile, info)
        old = self.by_uri.get(item.uri)
        if old is not None:
            ok, pos = self.store.find(old)
            if ok:
                item.texture = old.texture
                item.thumb_state = 0 if item.mtime != old.mtime else old.thumb_state
                was_sel = old in self.selected_items()
                self.store.splice(pos, 1, [item])
                if was_sel:
                    self.pending_select.add(item.uri)
        else:
            self.store.append(item)
        self.by_uri[item.uri] = item
        self._select_pending([item])
        self._update_empty()
        self.emit("contents-changed")

    def _remove(self, gfile):
        item = self.by_uri.pop(gfile.get_uri(), None)
        if item is None:
            return
        ok, pos = self.store.find(item)
        if ok:
            self.store.remove(pos)
        self._update_empty()
        self.emit("contents-changed")

    # ------------------------------------------------------------ search

    SEARCH_FLUSH_MS = 200      # hand results to the UI at most this often
    SEARCH_FLUSH_MAX = 250     # and at most this many per main-loop turn

    def search(self, root, query, show_hidden=False):
        self._reset("search")
        self.gfile = Gio.File.new_for_path(root)
        self.loading = True
        self._update_empty()
        stop = threading.Event()
        self._search_stop = stop
        gen = self._gen
        pending = []           # (gfile, info, location) from the worker
        lock = threading.Lock()
        state = {"done": False}

        def worker():
            for path in fc.search_walk(root, query, stop, show_hidden):
                g = Gio.File.new_for_path(path)
                try:
                    info = g.query_info(ATTRS, Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, None)
                except GLib.Error:
                    continue
                ct = info.get_content_type() or ""
                if ct:
                    type_description(ct)   # warm the cache off the main thread
                with lock:
                    pending.append((g, info, os.path.dirname(path)))
            with lock:
                state["done"] = True

        def flush():
            if gen != self._gen:
                return False
            with lock:
                batch = pending[:self.SEARCH_FLUSH_MAX]
                del pending[:self.SEARCH_FLUSH_MAX]
                final = state["done"] and not pending
            self._search_batch(batch, gen, final)
            return not final

        GLib.timeout_add(self.SEARCH_FLUSH_MS, flush)
        threading.Thread(target=worker, daemon=True, name="hb-search").start()

    def _search_batch(self, batch, gen, final):
        if gen != self._gen:
            return False
        items = []
        for g, info, loc in batch:
            it = FileItem(g, info, extra_location=loc)
            self.by_uri[it.uri] = it
            items.append(it)
        if items:
            self.store.splice(self.store.get_n_items(), 0, items)
        if final:
            self.loading = False
            self._search_stop = None
        if items or final:
            self._update_empty()
            self.emit("contents-changed")
        return False

    # ------------------------------------------------------------ state

    def _on_items_changed(self, *_):
        self._update_empty()

    def _update_empty(self):
        n = self.sort_model.get_n_items()
        text = None
        if self.error:
            text = "This location can't be shown.\n" + self.error
        elif n == 0 and not self.loading:
            text = {"trash": "The trash is empty.",
                    "search": "No items match your search."}.get(self.mode, "This folder is empty.")
        elif n == 0 and self.loading:
            text = "Searching…" if self.mode == "search" else "Working on it…"
        self.empty.set_visible(text is not None)
        if text:
            self.empty_label.set_label(text)

    def items(self):
        return [self.sort_model.get_item(i) for i in range(self.sort_model.get_n_items())]

    def selected_items(self):
        bs = self.selection.get_selection()
        out = []
        for i in range(bs.get_size()):
            it = self.sort_model.get_item(bs.get_nth(i))
            if it is not None:
                out.append(it)
        return out

    def position_of(self, item):
        for i in range(self.sort_model.get_n_items()):
            if self.sort_model.get_item(i) is item:
                return i
        return -1

    def select_item(self, item, extend=False):
        pos = self.position_of(item)
        if pos >= 0:
            self.selection.select_item(pos, not extend)

    def select_paths(self, paths):
        """Select these (now or once they show up through the monitor)."""
        uris = {Gio.File.new_for_path(p).get_uri() for p in paths}
        self.pending_select |= uris
        present = [self.by_uri[u] for u in uris if u in self.by_uri]
        if present:
            self.selection.unselect_all()
        self._select_pending(present)

    def _select_pending(self, items):
        if not self.pending_select:
            return
        for it in items:
            if it.uri in self.pending_select:
                self.pending_select.discard(it.uri)
                pos = self.position_of(it)
                if pos >= 0:
                    self.selection.select_item(pos, False)
                    self.scroll_to(pos)

    def scroll_to(self, pos):
        try:
            if self.view == "details":
                self.cv.scroll_to(pos, None, Gtk.ListScrollFlags.FOCUS, None)
            else:
                self.grid.scroll_to(pos, Gtk.ListScrollFlags.FOCUS, None)
        except (TypeError, AttributeError):
            pass

    def select_all(self):
        self.selection.select_all()

    def select_none(self):
        self.selection.unselect_all()

    def invert_selection(self):
        n = self.sort_model.get_n_items()
        cur = self.selection.get_selection()
        new = Gtk.Bitset.new_range(0, n)
        new.difference(cur)
        self.selection.set_selection(new, Gtk.Bitset.new_range(0, n))

    def focus_view(self):
        (self.cv if self.view == "details" else self.grid).grab_focus()

    def item_widget(self, item):
        for w in item.cells:
            if w.get_mapped():
                return w
        return None

    # ------------------------------------------------------------ input

    def _on_activate(self, _view, pos):
        item = self.sort_model.get_item(pos)
        if item is not None:
            self.emit("item-activated", item)

    def _on_click(self, gesture, n_press, x, y, view):
        button = gesture.get_current_button()
        picked = view.pick(x, y, Gtk.PickFlags.DEFAULT)
        if button == 1:
            # a click on empty space clears the selection (not on the column header)
            if find_item_widget(picked, view) is None and not _in_header(picked, view):
                self.selection.unselect_all()
            return
        if button not in (2, 3):
            return
        if _in_header(picked, view):
            return
        item = find_item_widget(picked, view)
        gesture.set_state(Gtk.EventSequenceState.CLAIMED)
        if button == 2:
            if item is not None:
                self.emit("middle-click", item)
            return
        if item is not None:
            if item not in self.selected_items():
                self.select_item(item)
        else:
            self.selection.unselect_all()
        px, py = widget_point(view, self, x, y)
        self.emit("context-menu", px, py, item)


def _in_header(widget, root):
    w = widget
    while w is not None and w is not root:
        if w.get_css_name() in ("header", "row", "child", "scrollbar"):
            return w.get_css_name() in ("header", "scrollbar")
        w = w.get_parent()
    return False


def pick_drop_action(drop, dest, files=None):
    """MOVE on the same file system, COPY otherwise; Ctrl forces copy."""
    if drop is None:
        return Gdk.DragAction.COPY
    actions = drop.get_actions()
    ctrl = False
    try:
        dev = drop.get_device()
        ctrl = bool(dev.get_modifier_state() & Gdk.ModifierType.CONTROL_MASK)
    except (AttributeError, TypeError):
        pass
    if ctrl and actions & Gdk.DragAction.COPY:
        return Gdk.DragAction.COPY
    same = True
    if files:
        same = all(same_filesystem(f, dest) for f in files)
    want = Gdk.DragAction.MOVE if same else Gdk.DragAction.COPY
    if actions & want:
        return want
    return Gdk.DragAction.COPY if actions & Gdk.DragAction.COPY else 0


_FS_IDS = {}          # uri -> (time, filesystem id); drag motion asks many times a second
_FS_TTL = 30.0


def _fs_id(g, flags):
    uri = g.get_uri()
    now = time.monotonic()
    hit = _FS_IDS.get(uri)
    if hit and now - hit[0] < _FS_TTL:
        return hit[1]
    info = g.query_info("id::filesystem", flags, None)
    fid = info.get_attribute_string("id::filesystem")
    if len(_FS_IDS) > 512:
        _FS_IDS.clear()
    _FS_IDS[uri] = (now, fid)
    return fid


def same_filesystem(a, b):
    try:
        return _fs_id(a, Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS) == \
            _fs_id(b, Gio.FileQueryInfoFlags.NONE)
    except GLib.Error:
        return False
