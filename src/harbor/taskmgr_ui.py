"""Shared table, page and dialog helpers for Harbor Task Manager."""

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, GLib, GObject, Gtk, Pango  # noqa: E402

from .common import heat_class, label  # noqa: E402

DEFAULT_ICON = "application-x-executable-symbolic"

CSS = """
.hb-tm-navcol { background: $window; }
.hb-tm-page-title { font-size: 22px; font-weight: 600; }
.hb-tm-page-head { min-height: 40px; }
.hb-tm-sub { font-size: 12px; color: $dim; }
columnview.hb-tm { font-size: 13px; }
columnview.hb-tm > listview > row { min-height: 30px; padding: 0; }
columnview.hb-tm > listview > row > cell { padding: 0; }
columnview.hb-tm > header { border-bottom: none; box-shadow: none; }
columnview.hb-tm > header > button { padding: 6px 0 8px 0; border-radius: 0;
    border-right: 1px solid $divider; color: $text; font-weight: normal; }
columnview.hb-tm > header > button:hover { background: $nav_hover; }
columnview.hb-tm > header > button > box { padding: 0 12px; }
columnview.hb-tm > header > button label { color: $text; opacity: 1; }
columnview.hb-tm > header > button:last-child { border-right: none; }
.hb-tm-cell { padding: 0 12px; min-height: 30px; }
.hb-tm-namecell { padding: 0 8px 0 10px; min-height: 30px; }
.hb-tm-group { font-weight: 600; font-size: 13px; padding-top: 4px; }
.hb-tm-dim { color: $dim; }
.hb-tm-status-icon { color: #4ca85a; }
.hb-tm-devlist, .hb-tm-devlist > row { background: transparent; }
.hb-tm-devlist > row { border-radius: 6px; margin: 1px 6px; padding: 6px 6px; }
.hb-tm-devlist > row:hover { background: $nav_hover; }
.hb-tm-devlist > row:selected { background: $nav_sel; color: $text; }
.hb-tm-devname { font-size: 13.5px; }
.hb-tm-devval { font-size: 12px; color: $dim; }
.hb-tm-perf-title { font-size: 22px; font-weight: 600; }
.hb-tm-perf-model { font-size: 15px; }
.hb-tm-perf-sub { font-size: 15px; font-weight: 600; }
.hb-tm-axis { font-size: 11.5px; color: $dim; }
.hb-tm-stat-label { font-size: 12px; color: $dim; }
.hb-tm-stat-value { font-size: 19px; }
.hb-tm-kv-label { font-size: 12.5px; color: $dim; }
.hb-tm-kv-value { font-size: 12.5px; }
.hb-tm-divider-v { background: $divider; min-width: 1px; }
.hb-tm-nav-collapsed .hb-nav-label { opacity: 0; }
.hb-tm-legend { min-width: 10px; min-height: 10px; border-radius: 2px; }
.hb-tm-legend-solid { background: alpha(var(--accent-bg-color), 0.6); }
.hb-tm-legend-cached { background: alpha(var(--accent-bg-color), 0.2);
    border: 1px solid alpha(var(--accent-bg-color), 0.6); }
.hb-tm-legend-free { border: 1px solid alpha(var(--accent-bg-color), 0.6); }
.hb-tm-pane { background: $card; border: 1px solid $border; border-radius: 6px; }
.hb-tm-empty { color: $dim; font-size: 13px; }
.hb-tm-dialog-key { color: $dim; }

/* Memory Sweep page, memory optimization settings, Swap view */
progressbar.hb-tm-bar > trough { min-height: 6px; min-width: 40px; border-radius: 3px;
    background: $divider; border: none; }
progressbar.hb-tm-bar > trough > progress { min-height: 6px; border-radius: 3px;
    background: var(--accent-bg-color); border: none; }
progressbar.hb-tm-bar.hb-tm-bar-thin > trough,
progressbar.hb-tm-bar.hb-tm-bar-thin > trough > progress { min-height: 4px; }
progressbar.hb-tm-bar.hb-tm-bar-warn > trough > progress { background: #c42b1c; }
progressbar.hb-tm-bar.hb-tm-bar-busy > trough > progress { background: #d08a00; }
.hb-tm-sweep-hero { background: $card; border: 1px solid $border; border-radius: 6px;
    padding: 16px 18px 18px 18px; }
.hb-tm-sweep-big { font-size: 26px; font-weight: 600; }
.hb-tm-sweep-of { font-size: 14px; color: $dim; }
.hb-tm-sweep-tile { background: $card; border: 1px solid $border; border-radius: 6px;
    padding: 12px 14px 14px 14px; }
.hb-tm-sweep-tile-title { font-size: 12.5px; color: $dim; }
.hb-tm-sweep-tile-value { font-size: 17px; font-weight: 600; }
.hb-tm-sweep-intro { font-size: 13px; color: $dim; margin-bottom: 6px; }
.hb-tm-sweep-size { font-size: 13px; font-feature-settings: "tnum"; }
.hb-tm-pill { font-size: 11.5px; border-radius: 10px; padding: 1px 8px;
    background: alpha(var(--accent-bg-color), 0.16); color: var(--accent-color); }
.hb-tm-pill-warn { background: rgba(208,138,0,0.18); color: #b07400; }
.hb-tm-ok { color: #4ca85a; }
.hb-tm-warn-text { color: #c42b1c; }
.hb-tm-busy-text { color: #d08a00; }
.hb-tm-plan { font-size: 13px; }
.hb-tm-note { font-size: 12px; color: $dim; }
spinbutton.hb-spin { background: $entry; border: 1px solid $entry_border;
    border-radius: 5px; min-height: 30px; box-shadow: none; }
spinbutton.hb-spin > text { min-width: 44px; padding: 0 6px; }
spinbutton.hb-spin > button { background: none; border: none; box-shadow: none;
    min-width: 28px; padding: 0; border-radius: 4px; }
spinbutton.hb-spin > button:hover { background: $nav_hover; }
.hb-tm-unit { color: $dim; font-size: 13px; min-width: 66px; }
.hb-tm-th { font-size: 12px; color: $dim; }
.hb-tm-td { font-size: 12.5px; font-feature-settings: "tnum"; }
"""


# ------------------------------------------------------------------ small widgets

def tool_button(icon, text, callback=None, tooltip=None):
    box = Gtk.Box(spacing=8)
    img = Gtk.Image.new_from_icon_name(icon)
    img.set_pixel_size(16)
    box.append(img)
    lab = Gtk.Label(label=text)
    box.append(lab)
    btn = Gtk.Button(child=box, css_classes=["hb-tool"])
    btn.text_label = lab
    btn.icon_image = img
    if tooltip:
        btn.set_tooltip_text(tooltip)
    if callback:
        btn.connect("clicked", lambda *_: callback())
    return btn


def menu_tool_button(icon, text, menu):
    box = Gtk.Box(spacing=8)
    img = Gtk.Image.new_from_icon_name(icon)
    img.set_pixel_size(16)
    box.append(img)
    box.append(Gtk.Label(label=text))
    btn = Gtk.MenuButton(child=box, css_classes=["hb-tool"], menu_model=menu)
    btn.set_has_frame(False)
    btn.set_always_show_arrow(False)
    return btn


def spin(lower, upper, step, value, on_change=None, digits=0):
    """Win11-style number box (Gtk.SpinButton) with a callback(int)."""
    adj = Gtk.Adjustment(lower=lower, upper=upper, step_increment=step,
                         page_increment=step * 5, value=value)
    sb = Gtk.SpinButton(adjustment=adj, numeric=True, digits=digits, width_chars=4,
                        valign=Gtk.Align.CENTER, css_classes=["hb-spin"])
    sb._quiet = False
    if on_change:
        sb.connect("value-changed", lambda s: None if s._quiet else on_change(
            int(round(s.get_value()))))

    def set_quiet(v):
        sb._quiet = True
        sb.set_value(v)
        sb._quiet = False
    sb.set_quiet = set_quiet
    return sb


def bar(fraction=0.0, thin=False):
    """Thin Win11 storage-style usage bar."""
    pb = Gtk.ProgressBar(css_classes=["hb-tm-bar"] + (["hb-tm-bar-thin"] if thin else []),
                         valign=Gtk.Align.CENTER)
    set_bar(pb, fraction)
    return pb


def set_bar(pb, fraction, warn_at=0.9, busy_at=None):
    f = max(0.0, min(1.0, fraction or 0.0))
    pb.set_fraction(f)
    for cls, on in (("hb-tm-bar-warn", f >= warn_at),
                    ("hb-tm-bar-busy", busy_at is not None and busy_at <= f < warn_at)):
        if on:
            pb.add_css_class(cls)
        else:
            pb.remove_css_class(cls)


def scroll_to_widget(scroller, widget, offset=8):
    """Scroll a Gtk.ScrolledWindow so widget is at the top (after layout)."""
    def go():
        child = scroller.get_child()
        if child is None:
            return False
        y = None
        try:
            gi.require_version("Graphene", "1.0")
            from gi.repository import Graphene
            ok, pt = widget.compute_point(child, Graphene.Point().init(0, 0))
            if ok:
                y = pt.y
        except (TypeError, AttributeError, ValueError):
            res = widget.translate_coordinates(child, 0, 0)
            if res and res[0]:
                y = res[2]
        if y is not None:
            adj = scroller.get_vadjustment()
            # y is relative to the viewport, which is already scrolled by value
            top = adj.get_value() + y - offset
            adj.set_value(max(0.0, min(top, adj.get_upper() - adj.get_page_size())))
        return False
    # Once layout is done, and again after a page switch has settled focus
    # (the viewport scrolls to the focused widget).
    GLib.timeout_add(60, go)
    GLib.timeout_add(320, go)


def set_icon(img, item_icon, icon_name):
    if item_icon is not None:
        img.set_from_gicon(item_icon)
    else:
        img.set_from_icon_name(icon_name or DEFAULT_ICON)


def copy_text(widget, text):
    clip = widget.get_clipboard()
    clip.set_content(Gdk.ContentProvider.new_for_value(GObject.Value(str, text)))


# ------------------------------------------------------------------ tables

class Col:
    """Column spec. kind: "name" (expander + icon + label), "text", "num".
    text(item) -> str; heat(item) -> fraction or None; key(item) -> sort key."""

    def __init__(self, key, title, text, width=None, expand=False, kind="text", heat=None,
                 sort=None, dim=None, resizable=True):
        self.key = key
        self.title = title
        self.text = text
        self.width = width
        self.expand = expand
        self.kind = kind
        self.heat = heat
        self.sort = sort
        self.dim = dim
        self.resizable = resizable


def _item_of(obj):
    if isinstance(obj, Gtk.TreeListRow):
        return obj, obj.get_item()
    return None, obj


class CellFactory(Gtk.SignalListItemFactory):
    def __init__(self, col, table):
        super().__init__()
        self.col = col
        self.table = table
        self.connect("setup", self._setup)
        self.connect("bind", self._bind)
        self.connect("unbind", self._unbind)

    def _setup(self, _f, li):
        col = self.col
        if col.kind == "name":
            exp = Gtk.TreeExpander(css_classes=["hb-tm-namecell"])
            exp.set_indent_for_depth(False)
            box = Gtk.Box(spacing=10)
            img = Gtk.Image(pixel_size=16)
            lab = Gtk.Label(xalign=0, ellipsize=Pango.EllipsizeMode.END, hexpand=True)
            box.append(img)
            box.append(lab)
            exp.set_child(box)
            li.set_child(exp)
            li._tm = (exp, img, lab)
            exp._tm_li = li
        elif col.kind == "status":
            box = Gtk.Box(spacing=6, css_classes=["hb-tm-cell"])
            img = Gtk.Image(pixel_size=14, css_classes=["hb-tm-status-icon"])
            lab = Gtk.Label(xalign=0, ellipsize=Pango.EllipsizeMode.END, hexpand=True)
            box.append(img)
            box.append(lab)
            li.set_child(box)
            li._tm = (None, img, lab)
            box._tm_li = li
        else:
            lab = Gtk.Label(xalign=1.0 if col.kind == "num" else 0.0,
                            ellipsize=Pango.EllipsizeMode.END, css_classes=["hb-tm-cell"])
            li.set_child(lab)
            li._tm = (None, None, lab)
            lab._tm_li = li
        li._tm_heat = None
        li._tm_bound = None

    def _bind(self, _f, li):
        row, item = _item_of(li.get_item())
        exp = li._tm[0]
        if exp is not None and row is not None:
            exp.set_list_row(row)
        hid = item.connect("changed", self._changed, li)
        li._tm_bound = (item, hid)
        self._refresh(li, item, row)

    def _unbind(self, _f, li):
        b = li._tm_bound
        if b:
            b[0].disconnect(b[1])
        li._tm_bound = None
        exp = li._tm[0]
        if exp is not None:
            exp.set_list_row(None)

    def _changed(self, item, li):
        row, _ = _item_of(li.get_item())
        self._refresh(li, item, row)

    def _refresh(self, li, item, row):
        exp, img, lab = li._tm
        col = self.col
        group = getattr(item, "kind", "") == "group"
        if col.kind == "name":
            lab.set_label(col.text(item))
            if group:
                lab.add_css_class("hb-tm-group")
                img.set_visible(False)
                exp.set_hide_expander(True)
                exp.set_indent_for_icon(False)
                exp.set_margin_start(0)
            else:
                lab.remove_css_class("hb-tm-group")
                img.set_visible(True)
                set_icon(img, getattr(item, "icon", None), getattr(item, "icon_name", None))
                exp.set_hide_expander(False)
                exp.set_indent_for_icon(True)
                depth = row.get_depth() if row is not None else 0
                depth -= self.table.base_depth
                exp.set_margin_start(26 * max(0, depth))
            return
        if group:
            lab.set_label("")
            if img is not None:
                img.set_visible(False)
            self._set_heat(li, lab, None)
            return
        text = col.text(item)
        if col.kind == "status":
            icon = None
            if text == "Efficiency mode":
                icon = "emoji-nature-symbolic"
            elif text == "Suspended":
                icon = "media-playback-pause-symbolic"
            img.set_visible(icon is not None)
            if icon:
                img.set_from_icon_name(icon)
        lab.set_label(text)
        if col.dim is not None:
            if col.dim(item):
                lab.add_css_class("hb-tm-dim")
            else:
                lab.remove_css_class("hb-tm-dim")
        self._set_heat(li, lab, heat_class(col.heat(item)) if col.heat else None)

    @staticmethod
    def _set_heat(li, widget, cls):
        if li._tm_heat == cls:
            return
        if li._tm_heat:
            widget.remove_css_class(li._tm_heat)
        if cls:
            widget.add_css_class(cls)
        li._tm_heat = cls


class Table:
    """ColumnView with Harbor styling, a scroller and right-click support."""

    def __init__(self, model, cols, on_menu=None, on_activate=None, base_depth=0):
        self.base_depth = base_depth
        self.selection = model
        self.cv = Gtk.ColumnView(model=model, css_classes=["hb-table", "hb-tm"],
                                 show_row_separators=False, show_column_separators=False,
                                 reorderable=False, hexpand=True, vexpand=True)
        self.columns = {}
        self.specs = {}
        for c in cols:
            col = Gtk.ColumnViewColumn(title=c.title, factory=CellFactory(c, self))
            if c.width:
                col.set_fixed_width(c.width)
            col.set_expand(c.expand)
            col.set_resizable(c.resizable)
            if c.sort is not None:
                col.set_sorter(c.sort)
            self.cv.append_column(col)
            self.columns[c.key] = col
            self.specs[c.key] = c
        self.scroller = Gtk.ScrolledWindow(child=self.cv, vexpand=True, hexpand=True)
        self.scroller.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.AUTOMATIC)
        self.on_menu = on_menu
        if on_menu:
            g = Gtk.GestureClick(button=3)
            g.connect("pressed", self._pressed)
            self.cv.add_controller(g)
            lp = Gtk.GestureLongPress(touch_only=True)
            lp.connect("pressed", lambda _g, x, y: self._pressed(None, 1, x, y))
            self.cv.add_controller(lp)
        if on_activate:
            self.cv.connect("activate", lambda _cv, pos: on_activate(pos))
        self._head = None
        self._sort_state = None
        self.cv.get_sorter().connect("changed", self._sort_clicked)

    def _sort_clicked(self, sorter, _change):
        """After the user picks another sort column, show the selected row
        (or the top of the list) instead of keeping a random scroll anchor."""
        state = (sorter.get_primary_sort_column(), sorter.get_primary_sort_order())
        if state == self._sort_state:
            return
        first = self._sort_state is None
        self._sort_state = state
        if first:
            return

        def scroll():
            pos = self.selection.get_selected() if hasattr(self.selection, "get_selected") \
                else Gtk.INVALID_LIST_POSITION
            if pos == Gtk.INVALID_LIST_POSITION:
                pos = 0
            if self.selection.get_n_items() > 0:
                self.cv.scroll_to(pos, None, Gtk.ListScrollFlags.NONE, None)
            return False
        GLib.idle_add(scroll)

    def _pressed(self, _g, _n, x, y):
        w = self.cv.pick(x, y, Gtk.PickFlags.DEFAULT)
        li = None
        while w is not None and w is not self.cv:
            li = getattr(w, "_tm_li", None)
            if li is not None:
                break
            w = w.get_parent()
        if li is None:
            return
        pos = li.get_position()
        if pos != Gtk.INVALID_LIST_POSITION:
            self.selection.select_item(pos, True)
        self.on_menu(self.cv, x, y)

    def header_labels(self):
        """{column key: Gtk.Label} of the header titles (for big totals)."""
        if self._head:
            return self._head
        head = self.cv.get_first_child()
        while head is not None and head.get_css_name() != "header":
            head = head.get_next_sibling()
        if head is None:
            return {}
        found = []

        def walk(w):
            while w is not None:
                if isinstance(w, Gtk.Label):
                    found.append(w)
                walk(w.get_first_child())
                w = w.get_next_sibling()
        walk(head.get_first_child())
        by_title = {}
        for lab in found:
            by_title.setdefault(lab.get_label(), lab)
        out = {}
        for key, col in self.columns.items():
            lab = by_title.get(col.get_title())
            if lab is not None:
                out[key] = lab
        if len(out) == len(self.columns):
            self._head = out
        return out

    def set_header(self, key, big, small, right=True):
        lab = self.header_labels().get(key)
        if lab is None:
            return
        if big is None:
            markup = "<span size='9.5pt' alpha='70%%'>%s</span>" % GLib.markup_escape_text(small)
        else:
            markup = ("<span size='13.5pt' weight='600' line_height='1.15'>%s</span>\n"
                      "<span size='9pt' alpha='70%%'>%s</span>" % (
                          GLib.markup_escape_text(big), GLib.markup_escape_text(small)))
        if lab.get_label() != markup:
            lab.set_use_markup(True)
            lab.set_label(markup)
        lab.set_xalign(1.0 if right else 0.0)
        lab.set_justify(Gtk.Justification.RIGHT if right else Gtk.Justification.LEFT)
        lab.set_hexpand(True)

    def selected_item(self):
        obj = self.selection.get_selected_item()
        if obj is None:
            return None
        return _item_of(obj)[1]

    def selected_row(self):
        obj = self.selection.get_selected_item()
        return obj if isinstance(obj, Gtk.TreeListRow) else None

    def popup(self, menu, x, y):
        pop = getattr(self, "_popover", None)
        if pop is None:
            pop = Gtk.PopoverMenu.new_from_model(menu)
            pop.set_has_arrow(False)
            pop.set_halign(Gtk.Align.START)
            pop.set_parent(self.cv)
            self._popover = pop
        else:
            pop.set_menu_model(menu)
        rect = Gdk.Rectangle()
        rect.x, rect.y, rect.width, rect.height = int(x), int(y), 1, 1
        pop.set_pointing_to(rect)
        pop.popup()


def cmp(a, b):
    return (a > b) - (a < b)


def key_sorter(keyfunc):
    return Gtk.CustomSorter.new(lambda a, b, *_: cmp(keyfunc(a), keyfunc(b)))


# ------------------------------------------------------------------ pages

class Page:
    key = ""
    title = ""

    def __init__(self, win):
        self.win = win
        self.app = win.get_application()
        self.box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        head = Gtk.Box(spacing=4, margin_start=20, margin_end=14, margin_top=10,
                       margin_bottom=4, css_classes=["hb-tm-page-head"])
        self.title_label = label(self.title, "hb-tm-page-title", valign=Gtk.Align.CENTER)
        head.append(self.title_label)
        head.append(Gtk.Box(hexpand=True))
        self.actions = Gtk.Box(spacing=2, valign=Gtk.Align.CENTER)
        head.append(self.actions)
        self.box.append(head)

    def on_tick(self):
        pass

    def on_show(self, sub=None):
        pass

    def on_search(self, text):
        pass


# ------------------------------------------------------------------ dialogs

def confirm(parent, heading, body, action_label, callback, destructive=True):
    dlg = Adw.AlertDialog(heading=heading, body=body)
    dlg.add_response("cancel", "Cancel")
    dlg.add_response("ok", action_label)
    dlg.set_default_response("cancel")
    dlg.set_close_response("cancel")
    if destructive:
        dlg.set_response_appearance("ok", Adw.ResponseAppearance.DESTRUCTIVE)
    else:
        dlg.set_response_appearance("ok", Adw.ResponseAppearance.SUGGESTED)
    dlg.connect("response", lambda _d, r: callback() if r == "ok" else None)
    dlg.present(parent)
    return dlg


def info_dialog(parent, title, rows):
    """Read-only property sheet: rows of (key, value)."""
    dlg = Adw.Dialog(title=title, content_width=520)
    tv = Adw.ToolbarView()
    tv.add_top_bar(Adw.HeaderBar())
    grid = Gtk.Grid(column_spacing=18, row_spacing=8, margin_start=24, margin_end=24,
                    margin_top=8, margin_bottom=24)
    for i, (k, v) in enumerate(rows):
        grid.attach(label(k, "hb-tm-dialog-key", valign=Gtk.Align.START), 0, i, 1, 1)
        val = label(str(v), selectable=True, wrap=True, hexpand=True, focusable=False)
        val.set_max_width_chars(60)
        grid.attach(val, 1, i, 1, 1)
    sc = Gtk.ScrolledWindow(child=grid, propagate_natural_height=True,
                            max_content_height=520)
    sc.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
    tv.set_content(sc)
    dlg.set_child(tv)
    dlg.present(parent)
    return dlg
