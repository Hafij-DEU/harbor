"""Harbor Files: one tab (history + pages + folder view) and the address bar."""

import os

from gi.repository import Gdk, Gio, GLib, GObject, Gtk

from . import files_core as fc
from .files_view import FolderView

SPECIAL_ICONS = {
    GLib.UserDirectory.DIRECTORY_DESKTOP: "user-desktop",
    GLib.UserDirectory.DIRECTORY_DOCUMENTS: "folder-documents",
    GLib.UserDirectory.DIRECTORY_DOWNLOAD: "folder-download",
    GLib.UserDirectory.DIRECTORY_PICTURES: "folder-pictures",
    GLib.UserDirectory.DIRECTORY_MUSIC: "folder-music",
    GLib.UserDirectory.DIRECTORY_VIDEOS: "folder-videos",
}


def icon_for_location(loc, searching=False):
    if searching:
        return "system-search"
    if loc == fc.HOME_PAGE:
        return "go-home"
    if loc == fc.THIS_PC:
        return "computer"
    if loc.startswith("trash:"):
        return "user-trash"
    if loc.startswith("network:"):
        return "network-workgroup"
    path = Gio.File.new_for_uri(loc).get_path()
    if not path:
        return "folder-remote"
    path = os.path.normpath(path)
    if path == os.path.normpath(fc.home_dir()):
        return "user-home"
    if path == "/":
        return "drive-harddisk"
    for which, icon in SPECIAL_ICONS.items():
        p = GLib.get_user_special_dir(which)
        if p and os.path.normpath(p) == path:
            return icon
    return "folder"


class Tab(Gtk.Box):
    """A tab: its own history, Home / This PC pages and a FolderView."""

    __gsignals__ = {"changed": (GObject.SignalFlags.RUN_FIRST, None, ())}

    def __init__(self, win, location):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, hexpand=True, vexpand=True)
        self.win = win
        s = win.app.settings
        self.history = []
        self.index = -1
        self.location = None
        self.search_query = None
        self.search_root = None
        self.writable = True
        self.page = None
        self.home_page = None
        self.thispc_page = None
        self.view = FolderView(show_hidden=s.get("show_hidden"), view=s.get("view"),
                               icon_size=s.get("icon_size"))
        self.view.set_show_extensions(bool(s.get("show_extensions")))
        self.view.set_sort(s.get("sort"), bool(s.get("descending")))
        v = self.view
        v.connect("item-activated", lambda _v, it: win.on_item_activated(self, it))
        v.connect("selection-changed", lambda _v: win.on_selection_changed(self))
        v.connect("context-menu", lambda _v, x, y, it: win.on_context_menu(self, x, y, it))
        v.connect("middle-click", lambda _v, it: win.on_middle_click(self, it))
        v.connect("contents-changed", lambda _v: win.on_contents_changed(self))
        v.connect("drop-files", lambda _v, files, dest, move: win.on_drop(self, files, dest, move))
        v.connect("needs-mount", lambda _v: self._mount())
        self.stack = Gtk.Stack(vexpand=True, hexpand=True,
                               transition_type=Gtk.StackTransitionType.NONE)
        self.stack.add_named(self.view, "view")
        self.append(self.stack)
        self.navigate(location)

    # ------------------------------------------------------------ state
    def kind(self):
        if self.search_query:
            return "search"
        k = fc.location_kind(self.location)
        if k == "network" and self.folder_path():
            return "folder"
        return k

    def folder_path(self):
        if self.location in (fc.HOME_PAGE, fc.THIS_PC):
            return None
        return Gio.File.new_for_uri(self.location).get_path()

    def title(self):
        base = fc.location_title(self.location)
        if self.search_query:
            if self.location in (fc.HOME_PAGE, fc.THIS_PC):
                base = "My files"
            return "Search results in %s" % base
        return base

    def icon(self):
        return icon_for_location(self.location, bool(self.search_query))

    def can_back(self):
        return self.index > 0 or bool(self.search_query)

    def can_forward(self):
        return self.index < len(self.history) - 1

    def can_up(self):
        return fc.parent_location(self.location) is not None and not self.search_query

    # ------------------------------------------------------------ navigation
    def navigate(self, location, record=True):
        if self.search_query:
            self.search_query = None
            self.view.stop_search()
        if record:
            if 0 <= self.index < len(self.history) and self.history[self.index] == location:
                pass
            else:
                del self.history[self.index + 1:]
                self.history.append(location)
                self.index = len(self.history) - 1
        self.location = location
        self._show()
        self.emit("changed")

    def _show(self):
        loc = self.location
        if loc == fc.HOME_PAGE:
            if self.home_page is None:
                from .files_pages import HomePage
                self.home_page = HomePage(self.win)
                self.stack.add_named(self.home_page, "home")
            else:
                self.home_page.refresh()
            self.stack.set_visible_child_name("home")
            return
        if loc == fc.THIS_PC:
            if self.thispc_page is None:
                from .files_pages import ThisPCPage
                self.thispc_page = ThisPCPage(self.win)
                self.stack.add_named(self.thispc_page, "thispc")
            else:
                self.thispc_page.refresh()
            self.stack.set_visible_child_name("thispc")
            return
        g = Gio.File.new_for_uri(loc)
        self.view.load(g, "trash" if loc.startswith("trash:") else "folder")
        self.stack.set_visible_child_name("view")
        self.writable = True
        if not loc.startswith("trash:"):
            def done(f, res, want=loc):
                try:
                    info = f.query_info_finish(res)
                except GLib.Error:
                    return
                if want == self.location and info.has_attribute("access::can-write"):
                    self.writable = info.get_attribute_boolean("access::can-write")
                    self.emit("changed")
            g.query_info_async("access::can-write", Gio.FileQueryInfoFlags.NONE,
                               GLib.PRIORITY_DEFAULT, None, done)

    def back(self):
        if self.search_query:
            self.end_search()
            return
        if self.index > 0:
            self.index -= 1
            self.navigate(self.history[self.index], record=False)

    def forward(self):
        if self.can_forward():
            self.index += 1
            self.navigate(self.history[self.index], record=False)

    def up(self):
        if self.search_query:
            self.end_search()
            return
        p = fc.parent_location(self.location)
        if p:
            child = self.folder_path()
            self.navigate(p)
            if child:
                self.view.select_paths([child])

    def refresh(self):
        if self.search_query:
            self.start_search(self.search_query)
        elif self.location == fc.HOME_PAGE and self.home_page:
            self.home_page.refresh()
        elif self.location == fc.THIS_PC and self.thispc_page:
            self.thispc_page.refresh()
        else:
            self.view.reload()
        self.emit("changed")

    def start_search(self, query):
        root = self.folder_path() or fc.home_dir()
        self.search_query = query
        self.search_root = root
        self.view.search(root, query, self.view.show_hidden)
        self.stack.set_visible_child_name("view")
        self.emit("changed")

    def end_search(self):
        if self.search_query is None:
            return
        self.search_query = None
        self.view.stop_search()
        self._show()
        self.emit("changed")

    def _mount(self):
        g = self.view.gfile
        if g is None:
            return
        op = Gtk.MountOperation(parent=self.win)

        def done(f, res):
            try:
                f.mount_enclosing_volume_finish(res)
            except GLib.Error as e:
                if not e.matches(Gio.io_error_quark(), Gio.IOErrorEnum.FAILED_HANDLED):
                    self.win.notify_toast("Can't open this location: " + e.message, timeout=6)
                    self.view.error = e.message
                    self.view._update_empty()
                return
            self.view.reload()
        g.mount_enclosing_volume(Gio.MountMountFlags.NONE, op, None, done)


# ---------------------------------------------------------------------- address bar

def crumbs_for(location, home=None):
    """[(label, location_or_None)] for the address bar."""
    home = home or fc.home_dir()
    if location == fc.HOME_PAGE:
        return [("Home", fc.HOME_PAGE)]
    if location == fc.THIS_PC:
        return [("This PC", fc.THIS_PC)]
    if location.startswith("trash:"):
        return [("Trash", fc.TRASH_URI)]
    g = Gio.File.new_for_uri(location)
    path = g.get_path()
    if path and location.startswith("file:"):
        out = []
        crumbs = fc.breadcrumbs(path, home)
        first_label, first_path = crumbs[0]
        if first_path == "/" and first_label != "Home":
            out.append(("This PC", fc.THIS_PC))
            out.append(("File System", fc.location_for("/")))
        else:
            out.append(("My files", fc.location_for(first_path)))
        for lab, p in crumbs[1:]:
            out.append((lab, fc.location_for(p)))
        return out
    chain = []
    while g is not None:
        chain.append(g)
        g = g.get_parent()
    out = []
    for i, f in enumerate(reversed(chain)):
        uri = f.get_uri()
        out.append((fc.location_title(uri) if i else fc.uri_to_label(uri).rstrip("/"), uri))
    return out


def collapse(crumbs, keep_tail=3, limit=5):
    """(visible, hidden): visible may contain a None marker for "…"."""
    if len(crumbs) <= limit:
        return list(crumbs), []
    hidden = crumbs[1:-keep_tail]
    return [crumbs[0], None] + crumbs[-keep_tail:], hidden


def edit_text(location):
    if location == fc.HOME_PAGE:
        return "Home"
    if location == fc.THIS_PC:
        return "This PC"
    g = Gio.File.new_for_uri(location)
    return g.get_path() or location


class AddressBar(Gtk.Stack):
    def __init__(self, win):
        super().__init__(hexpand=True, css_classes=["hbf-address"],
                         transition_type=Gtk.StackTransitionType.NONE)
        self.win = win
        self.tab = None
        box = Gtk.Box(spacing=2, css_classes=["hbf-address-box"])
        self.icon = Gtk.Image(pixel_size=16, margin_start=8, margin_end=4)
        box.append(self.icon)
        self.crumbs = Gtk.Box(spacing=0)
        box.append(self.crumbs)
        filler = Gtk.Box(hexpand=True)
        box.append(filler)
        click = Gtk.GestureClick()
        click.connect("released", lambda *_: self.edit())
        filler.add_controller(click)
        self.add_named(box, "crumbs")
        self.entry = Gtk.Entry(css_classes=["hbf-address-entry"], hexpand=True)
        self.entry.connect("activate", self._submit)
        key = Gtk.EventControllerKey()
        key.connect("key-pressed", self._key)
        self.entry.add_controller(key)
        focus = Gtk.EventControllerFocus()
        focus.connect("leave", lambda *_: GLib.idle_add(self._leave))
        self.entry.add_controller(focus)
        self.add_named(self.entry, "entry")
        self.set_visible_child_name("crumbs")

    def set_tab(self, tab):
        self.tab = tab
        self.set_visible_child_name("crumbs")
        while (c := self.crumbs.get_first_child()) is not None:
            self.crumbs.remove(c)
        if tab is None:
            return
        self.icon.set_from_icon_name(tab.icon() + ("" if tab.icon().endswith("-symbolic")
                                                   else "-symbolic"))
        crumbs = crumbs_for(tab.location)
        if tab.search_query:
            crumbs = crumbs + [("Search results", None)]
        visible, hidden = collapse(crumbs)
        for i, c in enumerate(visible):
            self.crumbs.append(Gtk.Image(icon_name="go-next-symbolic", pixel_size=10,
                                         css_classes=["hbf-chevron"]))
            if c is None:
                from .files_menus import item
                m = Gio.Menu()
                for lab, loc in hidden:
                    m.append_item(item(lab, "win.open-location", loc))
                btn = Gtk.MenuButton(label="…", menu_model=m, css_classes=["hbf-crumb"],
                                     has_frame=False, always_show_arrow=False)
                self.crumbs.append(btn)
                continue
            lab, loc = c
            btn = Gtk.Button(label=lab, css_classes=["hbf-crumb"], has_frame=False)
            btn.get_child().set_ellipsize(3)
            btn.get_child().set_max_width_chars(28)
            if loc is None:
                btn.set_sensitive(True)
                btn.connect("clicked", lambda *_: self.edit())
            else:
                btn.connect("clicked", lambda _b, l=loc: self.win.open_location(l))
                mid = Gtk.GestureClick(button=2)
                mid.connect("released", lambda *_a, l=loc: self.win.open_location(l, True))
                btn.add_controller(mid)
            self.crumbs.append(btn)

    def edit(self):
        if self.tab is None:
            return
        self.entry.set_text(edit_text(self.tab.location))
        self.set_visible_child_name("entry")
        self.entry.grab_focus()
        self.entry.select_region(0, -1)

    def _key(self, _c, keyval, _code, _state):
        if keyval == Gdk.KEY_Escape:
            self.cancel()
            return True
        return False

    def _leave(self):
        if self.get_visible_child_name() == "entry" and not self.entry.has_focus():
            self.set_visible_child_name("crumbs")
        return False

    def cancel(self):
        self.set_visible_child_name("crumbs")
        if self.tab is not None:
            self.tab.view.focus_view()

    def _submit(self, *_):
        text = self.entry.get_text()
        cwd = self.tab.folder_path() if self.tab else None
        ok = self.win.open_typed(text, cwd)
        if ok:
            self.set_visible_child_name("crumbs")
