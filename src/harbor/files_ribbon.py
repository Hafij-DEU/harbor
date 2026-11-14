"""Harbor Files: the Windows 10 style ribbon (Settings > Command bar).

The ribbon is described by RIBBON (pure data) and built by Ribbon. Buttons
use the same win.* actions as the Windows 11 command bar and the menus, so
they are enabled and disabled by the same ACTION_RULES in files.py.

Entry kinds:
  ("button", label, action, icon, size[, target])   size: "big" or "small"
  ("toggle", label, action, icon, size[, target])   stateful action
  ("check",  label, action)                         check box (bool action)
  ("menu",   label, menu_key, icon, size)           the model comes from a provider
"""

from gi.repository import Gio, GLib, Gtk, Pango

from . import files_icons

RIBBON = (
    ("File", (
        ("", (
            ("button", "Open new window", "win.new-window", "harbor-rb-window-new", "big"),
            ("button", "Open in Terminal", "win.terminal", "harbor-rb-terminal", "big"),
            ("button", "Options", "win.options", "harbor-rb-options", "big"),
            ("button", "Close", "win.close-window", "harbor-rb-close", "big"),
        )),
    )),
    ("Home", (
        ("Clipboard", (
            ("button", "Pin to Quick access", "win.pin-toggle", "harbor-rb-pin", "big"),
            ("button", "Copy", "win.copy", "harbor-rb-copy", "big"),
            ("button", "Paste", "win.paste", "harbor-rb-paste", "big"),
            ("button", "Cut", "win.cut", "harbor-rb-cut", "small"),
            ("button", "Copy path", "win.copy-path", "harbor-rb-link", "small"),
            ("button", "Paste shortcut", "win.paste-shortcut", "harbor-rb-shortcut", "small"),
        )),
        ("Organize", (
            ("menu", "Move to", "move-to", "harbor-rb-move-to", "big"),
            ("menu", "Copy to", "copy-to", "harbor-rb-copy-to", "big"),
            ("button", "Delete", "win.delete", "harbor-rb-delete", "big"),
            ("button", "Rename", "win.rename", "harbor-rb-rename", "big"),
        )),
        ("New", (
            ("button", "New folder", "win.new-folder", "harbor-rb-new-folder", "big"),
            ("menu", "New item", "new", "harbor-rb-new-item", "small"),
        )),
        ("Open", (
            ("button", "Properties", "win.properties", "harbor-rb-properties", "big"),
            ("button", "Open", "win.open", "harbor-rb-open", "small"),
            ("menu", "Open with", "open-with", "harbor-rb-open-with", "small"),
        )),
        ("Select", (
            ("button", "Select all", "win.select-all", "harbor-rb-select-all", "small"),
            ("button", "Select none", "win.select-none", "harbor-rb-select-none", "small"),
            ("button", "Invert selection", "win.invert-selection", "harbor-rb-invert",
             "small"),
        )),
    )),
    ("Share", (
        ("Send", (
            ("button", "Zip", "win.compress", "harbor-rb-zip", "big"),
            ("button", "Copy path", "win.copy-path", "harbor-rb-link", "big"),
        )),
        ("Wi-Fi", (
            ("button", "Share over Wi-Fi", "win.share-wifi", "harbor-rb-wifi", "big"),
            ("button", "Share selected", "win.share-folder", "harbor-rb-send", "small"),
            ("button", "Scan phone QR", "win.scan-phone", "harbor-rb-qr", "small"),
        )),
    )),
    ("View", (
        ("Panes", (
            ("toggle", "Navigation pane", "win.nav-pane", "harbor-rb-nav-pane", "big"),
            ("toggle", "Preview pane", "win.preview-pane", "harbor-rb-preview-pane", "small"),
            ("toggle", "Details pane", "win.details-pane", "harbor-rb-details-pane", "small"),
            ("toggle", "Two panes", "win.split-view", "harbor-rb-split", "big"),
        )),
        ("Layout", (
            ("toggle", "Large icons", "win.view", "harbor-rb-large-icons", "small", "large"),
            ("toggle", "Medium icons", "win.view", "harbor-rb-medium-icons", "small", "medium"),
            ("toggle", "Details", "win.view", "harbor-rb-details-view", "small", "details"),
        )),
        ("Current view", (
            ("menu", "Sort by", "sort", "harbor-rb-sort", "big"),
        )),
        ("Show/hide", (
            ("check", "File name extensions", "win.show-ext"),
            ("check", "Hidden items", "win.show-hidden"),
        )),
    )),
)

TABS = tuple(name for name, _groups in RIBBON)


def entries():
    """Every entry of the ribbon, flattened (for tests)."""
    for _tab, groups in RIBBON:
        for _group, items in groups:
            yield from items


def actions():
    """Action names (without "win.") used by ribbon buttons."""
    return {e[2].split(".", 1)[1] for e in entries() if e[0] != "menu"}


def menu_keys():
    return {e[2] for e in entries() if e[0] == "menu"}


# Custom design (hybrid bar): commands that keep a text label, and rarely used
# ones that move into the "More" menu at the end of the row
SIMPLE_LABELS = {"win.paste", "win.new-folder", "win.compress",
                 "win.share-wifi", "win.nav-pane", "win.split-view", "win.new-window",
                 "win.terminal"}
SIMPLE_MORE = {"win.pin-toggle", "win.copy-path", "win.paste-shortcut", "win.open",
               "win.select-all", "win.select-none", "win.invert-selection", "win.close-window"}
# Custom design: a standalone line of the most used commands under the ribbon
QUICK = (
    (("button", "Cut", "win.cut", "harbor-rb-cut", "small"),
     ("button", "Copy", "win.copy", "harbor-rb-copy", "small"),
     ("button", "Paste", "win.paste", "harbor-rb-paste", "small"),
     ("button", "Rename", "win.rename", "harbor-rb-rename", "small"),
     ("button", "Delete", "win.delete", "harbor-rb-delete", "small")),
    (("menu", "Move to", "move-to", "harbor-rb-move-to", "small"),
     ("menu", "Copy to", "copy-to", "harbor-rb-copy-to", "small")),
    (("button", "New folder", "win.new-folder", "harbor-rb-new-folder", "small"),
     ("menu", "New item", "new", "harbor-rb-new-item", "small")),
)
# what the quick line's Share menu holds (so the ribbon does not repeat them)
SHARE_MENU = (("Zip", "win.compress"), ("Copy path", "win.copy-path"),
              ("Share over Wi-Fi", "win.share-wifi"), ("Share selected", "win.share-folder"),
              ("Scan phone QR", "win.scan-phone"))
# always visible at the right end, whichever tab is open (like Windows 11)
SIMPLE_END = (
    ("menu", "Sort", "sort", "harbor-rb-sort", "small"),
    ("menu", "Share", "share", "harbor-rb-send", "small"),
    ("toggle", "Split", "win.split-view", "harbor-rb-split", "small"),
    ("toggle", "Details", "win.details-pane", "harbor-rb-details-pane", "small"),
    ("toggle", "Preview", "win.preview-pane", "harbor-rb-preview-pane", "small"),
)

# the Custom design swaps in its own round long-shadow set (harbor-rbc-*)
CUSTOM_SET = [False]


def _icon(name, size):
    img = Gtk.Image(pixel_size=size, css_classes=["hbf-ribbon-icon"])
    if CUSTOM_SET[0] and name.startswith("harbor-rb-"):
        name = "harbor-rbc-" + name[len("harbor-rb-"):]
    # Harbor's own full-colour ribbon set is used as it is, in every icon mode
    img.set_from_icon_name(name if name.startswith("harbor-rb-") else files_icons.themed(name))
    return img


def _find_label(w):
    if isinstance(w, Gtk.Label):
        return w
    c = w.get_first_child()
    while c is not None:
        r = _find_label(c)
        if r is not None:
            return r
        c = c.get_next_sibling()
    return None


class Ribbon(Gtk.Box):
    """The ribbon. menus: {menu_key: callable() -> Gio.MenuModel}."""

    def __init__(self, menus, collapsed=False, on_collapse=None, simplified=False,
                 custom=False):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, css_classes=["hbf-ribbon"]
                         + (["hbf-ribbon-simple"] if simplified else []))
        # simplified (Custom design): tabs and commands share one row, like the
        # Windows 11 command bar, but every ribbon command is still one click away
        self.simplified = bool(simplified)
        self.custom = bool(custom or simplified)
        CUSTOM_SET[0] = self.custom
        self.menus = menus
        self.on_collapse = on_collapse
        self.menu_buttons = {}
        self.tab_buttons = {}
        tabs = Gtk.Box(css_classes=["hbf-ribbon-tabs"])
        self.stack = Gtk.Stack(transition_type=Gtk.StackTransitionType.NONE, vhomogeneous=False)
        first = None
        for name, groups in RIBBON:
            if self.custom and not self.simplified and not self._custom_groups(groups):
                continue        # every command of this tab is in the quick line
            b = Gtk.ToggleButton(label=name, css_classes=["hbf-ribbon-tab"]
                                 + (["hbf-ribbon-file"] if name == "File" else []))
            if first is None:
                first = b
            else:
                b.set_group(first)
            b.connect("toggled", self._on_tab, name)
            b.connect("clicked", lambda btn: self.collapsed and btn.get_active()
                      and self.set_collapsed(False, notify=True))
            self.tab_buttons[name] = b
            tabs.append(b)
            self.stack.add_named(self._page(groups), name)
        tabs.append(Gtk.Box(hexpand=True))
        self.collapse_btn = Gtk.Button(icon_name="pan-up-symbolic", css_classes=["flat", "hbf-nav"],
                                       tooltip_text="Minimize the ribbon (Ctrl+F1)",
                                       valign=Gtk.Align.CENTER)
        self.collapse_btn.connect("clicked", lambda *_: self.set_collapsed(not self.collapsed,
                                                                           notify=True))
        tabs.append(self.collapse_btn)
        self.append(tabs)
        self.revealer = Gtk.Revealer(child=self.stack, reveal_child=True,
                                     transition_type=Gtk.RevealerTransitionType.NONE)
        if self.simplified:
            # one row: [tabs] | commands (scroll sideways when the window is narrow)
            self.remove(tabs)
            tabs.remove(self.collapse_btn)
            for c in list(tabs):
                if c.get_hexpand():
                    tabs.remove(c)
            tabs.set_valign(Gtk.Align.CENTER)
            # tabs on their own line, the commands in a strip under them (Win10-like)
            tabs.set_halign(Gtk.Align.START)
            tabs.add_css_class("hbf-ribbon-tabline")
            self.append(tabs)
            row = Gtk.Box(css_classes=["hbf-ribbon-row"])
            sw = Gtk.ScrolledWindow(child=self.revealer, hexpand=True,
                                    vscrollbar_policy=Gtk.PolicyType.NEVER,
                                    hscrollbar_policy=Gtk.PolicyType.EXTERNAL,
                                    propagate_natural_height=True)
            row.append(sw)
            end = Gtk.Box(css_classes=["hbf-cluster", "hbf-cluster-end"], spacing=2,
                          valign=Gtk.Align.CENTER)
            for e in SIMPLE_END:
                if e[0] == "menu" and e[2] not in self.menus:
                    continue
                b = self._button(e, simple=True)
                end.append(b)
            for b in end:
                lab = _find_label(b)
                if lab is not None:
                    lab.set_visible(True)
            row.append(end)
            self.append(row)
            wheel = Gtk.EventControllerScroll(
                flags=Gtk.EventControllerScrollFlags.BOTH_AXES)
            wheel.connect("scroll", self._on_wheel, sw)
            sw.add_controller(wheel)
        else:
            self.append(self.revealer)
            if self.custom:
                self.append(self._quick_line())
        self.collapsed = False
        self.select("Home")
        self.set_collapsed(collapsed and not self.simplified)

    # ------------------------------------------------------------ layout
    def _page(self, groups):
        if self.simplified:
            return self._simple_page(groups)
        if self.custom:
            groups = self._custom_groups(groups)
        page = Gtk.Box(css_classes=["hbf-ribbon-page"], spacing=0)
        for i, (caption, items) in enumerate(groups):
            if i:
                page.append(Gtk.Separator(orientation=Gtk.Orientation.VERTICAL,
                                          css_classes=["hbf-ribbon-sep"]))
            page.append(self._group(caption, items))
        return page

    def _simple_page(self, groups):
        """Hybrid row: each ribbon group is a soft rounded cluster; rare commands
        go into a "More" menu at the end so the row stays short."""
        page = Gtk.Box(css_classes=["hbf-ribbon-page"], spacing=8, valign=Gtk.Align.CENTER)
        more = Gio.Menu()
        for caption, items in groups:
            cluster = Gtk.Box(css_classes=["hbf-cluster"], spacing=2, valign=Gtk.Align.CENTER,
                              tooltip_text=caption or None)
            for e in items:
                if e[0] == "check":
                    cluster.append(Gtk.CheckButton(label=e[1], action_name=e[2],
                                                   css_classes=["hbf-ribbon-check"]))
                elif e[0] != "menu" and e[2] in SIMPLE_MORE:
                    it = Gio.MenuItem.new(e[1], e[2])
                    if len(e) > 5:
                        it.set_action_and_target_value(e[2], GLib.Variant("s", e[5]))
                    more.append_item(it)
                else:
                    cluster.append(self._button(e, simple=True))
            if cluster.get_first_child() is not None:
                page.append(cluster)
        if more.get_n_items():
            mb = Gtk.MenuButton(icon_name="view-more-symbolic", menu_model=more,
                                tooltip_text="More", css_classes=["hbf-ribbon-simple-btn",
                                                                  "hbf-cluster-more"],
                                valign=Gtk.Align.CENTER)
            mb.set_has_frame(False)
            page.append(mb)
        return page

    @staticmethod
    def _custom_groups(groups):
        """Ribbon groups minus what the quick line already has (no duplicates)."""
        quick = {e[2] for items in QUICK for e in items} | {e[2] for e in SIMPLE_END}
        quick |= {a for _l, a in SHARE_MENU}
        groups = [(c, [e for e in items if e[2] not in quick]) for c, items in groups]
        return [(c, items) for c, items in groups if items]

    def _quick_line(self):
        """Custom design: most used commands on their own line under the ribbon;
        it stays when the ribbon is minimized."""
        line = Gtk.Box(css_classes=["hbf-ribbon-row", "hbf-quick-line"], spacing=8)
        body = Gtk.Box(spacing=8, valign=Gtk.Align.CENTER)
        for items in QUICK:
            body.append(self._cluster(items))
        sw = Gtk.ScrolledWindow(child=body, hexpand=True,
                                vscrollbar_policy=Gtk.PolicyType.NEVER,
                                hscrollbar_policy=Gtk.PolicyType.EXTERNAL,
                                propagate_natural_height=True)
        wheel = Gtk.EventControllerScroll(flags=Gtk.EventControllerScrollFlags.BOTH_AXES)
        wheel.connect("scroll", self._on_wheel, sw)
        sw.add_controller(wheel)
        line.append(sw)
        end = self._cluster(e for e in SIMPLE_END if e[0] != "menu" or e[2] in self.menus)
        end.add_css_class("hbf-cluster-end")
        line.append(end)
        return line

    def _cluster(self, items):
        box = Gtk.Box(css_classes=["hbf-cluster"], spacing=2, valign=Gtk.Align.CENTER)
        for e in items:
            b = self._button(e, simple=True)
            lab = _find_label(b)
            if lab is not None and e[1] not in ("Cut", "Copy", "Rename", "Delete"):
                lab.set_visible(True)
            box.append(b)
        return box

    def _group(self, caption, items):
        g = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, css_classes=["hbf-ribbon-group"])
        row = Gtk.Box(spacing=2, vexpand=True)
        column = None
        for e in items:
            kind = e[0]
            if kind == "check":
                w = Gtk.CheckButton(label=e[1], action_name=e[2], css_classes=["hbf-ribbon-check"])
                big = False
            else:
                w = self._button(e)
                big = e[4] == "big"
            if big:
                row.append(w)
                column = None
            else:
                if column is None or column.n >= 3:
                    column = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0,
                                     valign=Gtk.Align.START)
                    column.n = 0
                    row.append(column)
                column.append(w)
                column.n += 1
        g.append(row)
        if caption:
            g.append(Gtk.Label(label=caption, css_classes=["hbf-ribbon-caption"]))
        return g

    def _button(self, e, simple=False):
        kind, label, action, icon, size = e[:5]
        big = size == "big"
        if simple:
            inner = Gtk.Box(spacing=6)
            inner.append(_icon(icon, 22))
            # labels only on the drop-downs (Move to, Copy to, New...), the
            # rest are icon buttons with tooltips so the whole tab fits one row
            lab = Gtk.Label(label=label, visible=kind == "menu" or action in SIMPLE_LABELS)
            big = False
        else:
            inner = Gtk.Box(orientation=Gtk.Orientation.VERTICAL if big
                            else Gtk.Orientation.HORIZONTAL, spacing=2 if big else 6)
            inner.append(_icon(icon, 32 if big else 16))
            lab = Gtk.Label(label=label, wrap=big, justify=Gtk.Justification.CENTER,
                            max_width_chars=10 if big else -1)
        lab.set_wrap_mode(Pango.WrapMode.WORD)
        inner.append(lab)
        if kind == "menu":
            arrow = Gtk.Image(icon_name="pan-down-symbolic", pixel_size=10,
                              css_classes=["hbf-ribbon-arrow"])
            if big:
                inner.append(arrow)
            else:
                lab.set_hexpand(True)
                lab.set_xalign(0)
                inner.append(arrow)
        cls = ["hbf-ribbon-btn", "hbf-ribbon-big" if big else "hbf-ribbon-small"]
        if simple:
            cls.append("hbf-ribbon-simple-btn")
        if kind == "menu":
            w = Gtk.MenuButton(child=inner, css_classes=cls, always_show_arrow=False)
            w.set_has_frame(False)
            provider = self.menus.get(action)
            w.set_create_popup_func(lambda mb, p=provider: mb.set_menu_model(p() if p else None))
            if provider is not None:
                w.set_menu_model(provider())
            self.menu_buttons[action] = w
            return w
        if kind == "toggle":
            w = Gtk.ToggleButton(child=inner, css_classes=cls, action_name=action)
        else:
            w = Gtk.Button(child=inner, css_classes=cls, action_name=action)
        if len(e) > 5:
            w.set_action_target_value(GLib.Variant("s", e[5]))
        w.set_tooltip_text(label)
        return w

    @staticmethod
    def _on_wheel(_ctl, dx, dy, sw):
        adj = sw.get_hadjustment()
        if adj.get_upper() - adj.get_page_size() <= 0:
            return False
        adj.set_value(adj.get_value() + (dx or dy) * 40)
        return True

    # ------------------------------------------------------------ state
    def _on_tab(self, button, name):
        if not button.get_active():
            return
        self.stack.set_visible_child_name(name)
        if self.collapsed:
            self.set_collapsed(False, notify=True)

    def select(self, name):
        b = self.tab_buttons.get(name)
        if b is not None:
            b.set_active(True)
            self.stack.set_visible_child_name(name)

    def current(self):
        return self.stack.get_visible_child_name()

    def set_collapsed(self, collapsed, notify=False):
        self.collapsed = bool(collapsed)
        self.revealer.set_reveal_child(not self.collapsed)
        self.collapse_btn.set_icon_name("pan-down-symbolic" if self.collapsed else "pan-up-symbolic")
        self.collapse_btn.set_tooltip_text("Expand the ribbon (Ctrl+F1)" if self.collapsed
                                           else "Minimize the ribbon (Ctrl+F1)")
        if notify and self.on_collapse:
            self.on_collapse(self.collapsed)

    def refresh_icons(self):
        """Rebuild after the icon set changed (cheap: the ribbon is small)."""
        current = self.current()
        for name, groups in RIBBON:
            if self.custom and not self.simplified and not self._custom_groups(groups):
                continue        # every command of this tab is in the quick line
            old = self.stack.get_child_by_name(name)
            self.stack.remove(old)
            self.stack.add_named(self._page(groups), name)
        self.select(current or "Home")
