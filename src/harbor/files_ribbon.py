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

from gi.repository import GLib, Gtk, Pango

from . import files_icons

RIBBON = (
    ("File", (
        ("", (
            ("button", "Open new window", "win.new-window", "window-new-symbolic", "big"),
            ("button", "Open in Terminal", "win.terminal", "utilities-terminal-symbolic", "big"),
            ("button", "Options", "win.options", "preferences-system-symbolic", "big"),
            ("button", "Close", "win.close-window", "window-close-symbolic", "big"),
        )),
    )),
    ("Home", (
        ("Clipboard", (
            ("button", "Pin to Quick access", "win.pin-toggle", "starred", "big"),
            ("button", "Copy", "win.copy", "edit-copy-symbolic", "big"),
            ("button", "Paste", "win.paste", "edit-paste-symbolic", "big"),
            ("button", "Cut", "win.cut", "edit-cut-symbolic", "small"),
            ("button", "Copy path", "win.copy-path", "insert-link-symbolic", "small"),
            ("button", "Paste shortcut", "win.paste-shortcut", "emblem-symbolic-link", "small"),
        )),
        ("Organize", (
            ("menu", "Move to", "move-to", "go-next-symbolic", "big"),
            ("menu", "Copy to", "copy-to", "edit-copy-symbolic", "big"),
            ("button", "Delete", "win.delete", "user-trash-symbolic", "big"),
            ("button", "Rename", "win.rename", "document-edit-symbolic", "big"),
        )),
        ("New", (
            ("button", "New folder", "win.new-folder", "folder", "big"),
            ("menu", "New item", "new", "document-new-symbolic", "small"),
        )),
        ("Open", (
            ("button", "Properties", "win.properties", "document-properties-symbolic", "big"),
            ("button", "Open", "win.open", "document-open-symbolic", "small"),
            ("menu", "Open with", "open-with", "system-run-symbolic", "small"),
        )),
        ("Select", (
            ("button", "Select all", "win.select-all", "edit-select-all-symbolic", "small"),
            ("button", "Select none", "win.select-none", "edit-clear-symbolic", "small"),
            ("button", "Invert selection", "win.invert-selection", "object-flip-horizontal-symbolic",
             "small"),
        )),
    )),
    ("Share", (
        ("Send", (
            ("button", "Zip", "win.compress", "package-x-generic-symbolic", "big"),
            ("button", "Copy path", "win.copy-path", "insert-link-symbolic", "big"),
        )),
        ("Wi-Fi", (
            ("button", "Share over Wi-Fi", "win.share-wifi", "network-wireless-symbolic", "big"),
            ("button", "Share selected", "win.share-folder", "send-to-symbolic", "small"),
            ("button", "Scan phone QR", "win.scan-phone", "camera-photo-symbolic", "small"),
        )),
    )),
    ("View", (
        ("Panes", (
            ("toggle", "Navigation pane", "win.nav-pane", "sidebar-show-symbolic", "big"),
            ("toggle", "Preview pane", "win.preview-pane", "view-paged-symbolic", "small"),
            ("toggle", "Details pane", "win.details-pane", "view-dual-symbolic", "small"),
        )),
        ("Layout", (
            ("toggle", "Large icons", "win.view", "view-grid-symbolic", "small", "large"),
            ("toggle", "Medium icons", "win.view", "view-app-grid-symbolic", "small", "medium"),
            ("toggle", "Details", "win.view", "view-list-symbolic", "small", "details"),
        )),
        ("Current view", (
            ("menu", "Sort by", "sort", "view-sort-descending-symbolic", "big"),
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


def _icon(name, size):
    img = Gtk.Image(pixel_size=size)
    img.set_from_icon_name(files_icons.themed(name))
    return img


class Ribbon(Gtk.Box):
    """The ribbon. menus: {menu_key: callable() -> Gio.MenuModel}."""

    def __init__(self, menus, collapsed=False, on_collapse=None):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, css_classes=["hbf-ribbon"])
        self.menus = menus
        self.on_collapse = on_collapse
        self.menu_buttons = {}
        self.tab_buttons = {}
        tabs = Gtk.Box(css_classes=["hbf-ribbon-tabs"])
        self.stack = Gtk.Stack(transition_type=Gtk.StackTransitionType.NONE, vhomogeneous=False)
        first = None
        for name, groups in RIBBON:
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
        self.append(self.revealer)
        self.collapsed = False
        self.select("Home")
        self.set_collapsed(collapsed)

    # ------------------------------------------------------------ layout
    def _page(self, groups):
        page = Gtk.Box(css_classes=["hbf-ribbon-page"], spacing=0)
        for i, (caption, items) in enumerate(groups):
            if i:
                page.append(Gtk.Separator(orientation=Gtk.Orientation.VERTICAL,
                                          css_classes=["hbf-ribbon-sep"]))
            page.append(self._group(caption, items))
        return page

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

    def _button(self, e):
        kind, label, action, icon, size = e[:5]
        big = size == "big"
        inner = Gtk.Box(orientation=Gtk.Orientation.VERTICAL if big else Gtk.Orientation.HORIZONTAL,
                        spacing=2 if big else 6)
        inner.append(_icon(icon, 32 if big else 16))
        lab = Gtk.Label(label=label, wrap=big, justify=Gtk.Justification.CENTER,
                        max_width_chars=10 if big else -1)
        lab.set_wrap_mode(Pango.WrapMode.WORD)
        inner.append(lab)
        cls = ["hbf-ribbon-btn", "hbf-ribbon-big" if big else "hbf-ribbon-small"]
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
            old = self.stack.get_child_by_name(name)
            self.stack.remove(old)
            self.stack.add_named(self._page(groups), name)
        self.select(current or "Home")
