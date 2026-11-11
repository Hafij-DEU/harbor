"""Harbor Files: a Windows 11 File Explorer style file manager for GNOME."""

import os
import sys
import time

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, GLib, Gtk  # noqa: E402

from . import common  # noqa: E402
from . import files_core as fc  # noqa: E402
from . import files_menus as fm  # noqa: E402
from . import files_sidebar as fsb  # noqa: E402
from .common import HarborWindow, apply_theme  # noqa: E402
from .files_ops import Operations  # noqa: E402
from .files_tab import AddressBar, Tab  # noqa: E402

APP_ID = "io.github.hafij_deu.Harbor.Files"
APP_NAME = "Harbor Files"
ICON = APP_ID

# Content types we never run on double-click (Explorer would; we refuse).
EXEC_TYPES = {
    "application/x-executable", "application/x-sharedlib", "application/x-pie-executable",
    "application/x-desktop", "application/vnd.appimage", "application/x-ms-dos-executable",
    "application/x-msdownload", "application/x-ms-ne-executable", "application/x-msi",
}
OPEN_CONFIRM_OVER = 10


# ---------------------------------------------------------------------- pure helpers

def is_executable_type(content_type):
    return bool(content_type) and content_type in EXEC_TYPES


def open_terminal(path, have=None):
    """Open a terminal in path (via common.spawn, so dry runs only log it)."""
    argv = fc.terminal_argv(path, have or common.have)
    if not argv:
        return False
    return common.spawn(argv)


def arg_to_location(arg, cwd=None):
    """Command-line argument -> (tab location, path to select or None)."""
    loc = fc.parse_location(arg, cwd)
    if loc is None:
        return None
    if loc.startswith("/"):
        if os.path.exists(loc) and not os.path.isdir(loc):
            return fc.location_for(os.path.dirname(loc) or "/"), loc
        return fc.location_for(loc), None
    return fc.location_for(loc), None


def parse_page_key(key):
    """show_page() key -> (kind, arg, extra)."""
    if key in ("home", "thispc", "trash", "details", "menu", "network", "newtab"):
        return key, None, None
    for prefix in ("folder:", "properties:", "menu:"):
        if key.startswith(prefix):
            return prefix[:-1], key[len(prefix):], None
    if key.startswith("search:"):
        rest = key[len("search:"):]
        path, sep, query = rest.rpartition(":")
        if not sep:
            return "search", None, rest
        return "search", path or None, query
    return None, None, None


def selection_size(items):
    """Size shown in the status bar: only files count; None when folders are in it."""
    if not items or any(i.is_dir for i in items):
        return None
    return sum(i.size for i in items)


def sel_dicts(items):
    return [dict(is_dir=i.is_dir, local=bool(i.path), can_rename=i.can("rename"),
                 can_trash=i.can("delete") or i.can("trash"),
                 archive=bool(fc.archive_kind(i.name)) and not i.is_dir,
                 content_type=i.content_type) for i in items]


def folder_location_of(item):
    """Where opening a folder item goes (mountables use their target)."""
    target = item.info.get_attribute_string("standard::target-uri") \
        if item.info.has_attribute("standard::target-uri") else None
    return target or item.uri


# Which window actions follow which fc.command_states() key.
ACTION_RULES = {
    "new-folder": "new", "new-text": "new", "new-empty": "new",
    "cut": "cut", "copy": "copy", "paste": "paste", "rename": "rename",
    "delete": "delete", "delete-permanent": "delete-permanent", "restore": "restore",
    "open": "open", "open-tab": "open-new-tab", "open-with-other": "open-with",
    "open-with-app": "open-with", "compress": "compress", "extract": "extract",
    "pin-toggle": "pin", "select-all": "select-all", "select-none": "select-none",
    "invert-selection": "invert-selection", "copy-path": "copy-path",
}


# ---------------------------------------------------------------------- CSS

CSS = """
.hbf-titlebar { background: $window; padding: 0 0 0 10px; min-height: 42px; }
.hbf-titlebar tabbar, .hbf-titlebar tabbar > revealer > box {
    background: transparent; box-shadow: none; border: none; padding: 0; }
.hbf-titlebar tabbar tabbox { padding: 6px 0 0 0; }
.hbf-titlebar tabbar tab { border-radius: 8px 8px 0 0; min-height: 34px; margin: 0;
    background: transparent; box-shadow: none; }
.hbf-titlebar tabbar tab:hover { background: $nav_hover; }
.hbf-titlebar tabbar tab:selected, .hbf-titlebar tabbar tab:selected:hover {
    background: $surface; box-shadow: none; }
.hbf-titlebar tabbar tab .tab-title { font-size: 12.5px; }
.hbf-titlebar tabbar tabbox > separator { opacity: 0.6; margin-top: 12px; margin-bottom: 6px; }
.hbf-titlebar windowcontrols button { min-height: 30px; min-width: 40px; }
button.hbf-newtab { min-height: 28px; min-width: 28px; margin: 6px 4px 0 4px; border-radius: 5px; }

.hbf-navbar { background: $surface; padding: 6px 10px; }
.hbf-cmdbar { background: $surface; padding: 4px 10px 6px 10px; border-bottom: 1px solid $divider; }
.hbf-cmdbar button.hb-tool, .hbf-cmdbar menubutton > button { min-height: 32px; }
.hbf-cmdbar .hbf-vsep { background: $divider; min-width: 1px; margin: 6px 6px; }
button.hbf-nav { background: none; border-radius: 5px; min-width: 32px; min-height: 32px; padding: 0; }
button.hbf-nav:hover { background: $nav_hover; }
button.hbf-nav:disabled { opacity: 0.35; }
menubutton.hbf-new > button { background: var(--accent-bg-color); color: var(--accent-fg-color);
    border-radius: 5px; padding: 4px 12px; }
menubutton.hbf-new > button:hover { filter: brightness(1.08); }
togglebutton.hb-tool:checked, button.hb-tool:checked { background: $nav_sel; }

.hbf-address { background: $entry; border: 1px solid $entry_border; border-radius: 5px;
    min-height: 32px; }
.hbf-address entry { background: transparent; border: none; box-shadow: none; outline: none;
    min-height: 30px; }
button.hbf-crumb, menubutton.hbf-crumb > button { background: none; border-radius: 4px;
    padding: 2px 6px; font-weight: normal; min-height: 26px; font-size: 13px; color: $text; }
button.hbf-crumb:hover, menubutton.hbf-crumb > button:hover { background: $nav_hover; }
.hbf-chevron { color: $faint; margin: 0 1px; }
searchentry.hbf-search { min-height: 32px; }

.hbf-body { background: $surface; }
.hbf-sidebar { background: $surface; border-right: 1px solid $divider; }
.hbf-sidebar list.hb-nav > row { min-height: 32px; }
.hbf-sidebar list.hb-nav > row.hbf-side-sep-row { min-height: 13px; padding: 0; background: none; }
.hbf-sidebar list.hb-nav > row.hbf-side-sep-row:hover { background: none; }
.hbf-pane { background: $surface; border-left: 1px solid $divider; }
.hbf-pane-title { font-size: 15px; font-weight: 600; }
.hbf-pane-value { font-size: 13px; }
separator.hbf-sep, .hbf-sep { background: $divider; min-height: 1px; }
.hbf-status { background: $surface; padding: 3px 14px; min-height: 24px;
    border-top: 1px solid $divider; font-size: 12px; color: $dim; }
.hbf-status progressbar trough, .hbf-status progressbar progress { min-height: 4px; }

.hbf-view { background: $surface; }
columnview.hbf-table > listview > row { min-height: 30px; }
columnview.hbf-table > listview > row > cell { padding: 0 8px; }
columnview.hbf-table > listview > row:hover { background: $nav_hover; }
columnview.hbf-table > listview > row:selected { background: alpha(var(--accent-bg-color), 0.24); color: $text; }
columnview.hbf-table > header > button { font-size: 12px; color: $dim; padding: 4px 8px;
    border-right: 1px solid $divider; }
gridview.hbf-grid { background: transparent; padding: 8px; }
gridview.hbf-grid > child { border-radius: 6px; padding: 6px 2px; margin: 2px; }
gridview.hbf-grid > child:hover { background: $nav_hover; }
gridview.hbf-grid > child:selected { background: alpha(var(--accent-bg-color), 0.28); color: $text; }
.hbf-tile-label { font-size: 12.5px; }
.hbf-cell { font-size: 13px; }
.hbf-name label { font-size: 13px; }

.hbf-page { background: $surface; }
.hbf-hello { font-size: 26px; font-weight: 600; }
.hbf-section { font-size: 14px; font-weight: 600; margin-top: 18px; margin-bottom: 6px; }
flowbox.hbf-tiles > flowboxchild { border-radius: 8px; padding: 0; }
flowbox.hbf-tiles > flowboxchild:hover { background: $nav_hover; }
flowbox.hbf-tiles > flowboxchild:selected { background: $nav_sel; }
.hbf-qtile { padding: 10px 12px; }
.hbf-qtile-title { font-size: 13.5px; }
list.hbf-recent { background: transparent; }
list.hbf-recent > row { border-radius: 5px; min-height: 36px; }
list.hbf-recent > row:hover { background: $nav_hover; }
list.hbf-recent > row:selected { background: $nav_sel; color: $text; }
.hbf-recent-head { font-size: 12px; color: $dim; }
progressbar.hbf-bar trough { min-height: 8px; border-radius: 4px; background: $divider; }
progressbar.hbf-bar progress { min-height: 8px; border-radius: 4px; background: var(--accent-bg-color); }
progressbar.hbf-bar-warn progress { background: #e74856; }
popover.hbf-rename > contents { padding: 4px; }
list.hbf-apps { background: transparent; }
list.hbf-apps > row { border-radius: 5px; }
.hbf-prop-name { font-size: 16px; font-weight: 600; }
"""


# ---------------------------------------------------------------------- window

class FilesWindow(HarborWindow):
    def __init__(self, app, locations=None):
        s = app.settings
        super().__init__(app, APP_NAME, width=int(s.get("width") or 1200),
                         height=int(s.get("height") or 760), icon=ICON)
        self.app = app
        self.settings = s
        self.toolbar.remove(self.header)
        if s.get("maximized"):
            self.maximize()
        self._popover = None
        self._search_guard = False
        self._typeahead = ("", 0.0)
        self._details_id = 0
        self._undo = []
        self._build()
        self.ops = Operations(self)
        self._install_actions()
        self._install_keys()
        for loc, sel in (locations or [(fc.HOME_PAGE, None)]):
            self.open_arg(loc, sel, new_tab=True)
        self.connect("close-request", self._on_close_request)
        self.update_actions()

    # ------------------------------------------------------------ layout
    def _build(self):
        # 1. tab strip (also the title bar)
        self.tab_view = Adw.TabView(hexpand=True, vexpand=True)
        sc = Adw.TabViewShortcuts
        self.tab_view.set_shortcuts(sc.CONTROL_TAB | sc.CONTROL_SHIFT_TAB | sc.CONTROL_PAGE_UP
                                    | sc.CONTROL_PAGE_DOWN | sc.ALT_DIGITS | sc.ALT_ZERO)
        self.tab_view.connect("notify::selected-page", lambda *_: self._on_switch())
        self.tab_view.connect("close-page", self._on_close_page)
        self.tab_bar = Adw.TabBar(view=self.tab_view, autohide=False, expand_tabs=False,
                                  hexpand=True)
        newtab = Gtk.Button(icon_name="list-add-symbolic", css_classes=["flat", "hbf-newtab"],
                            tooltip_text="New tab (Ctrl+T)", valign=Gtk.Align.CENTER)
        newtab.connect("clicked", lambda *_: self.new_tab())
        self.tab_bar.set_end_action_widget(newtab)
        tbox = Gtk.Box(css_classes=["hbf-titlebar"], spacing=6)
        icon = Gtk.Image(icon_name=ICON, pixel_size=16, valign=Gtk.Align.CENTER,
                         margin_top=4, margin_end=4)
        tbox.append(icon)
        tbox.append(self.tab_bar)
        ctl = Gtk.WindowControls(side=Gtk.PackType.END, decoration_layout=":minimize,maximize,close",
                                 valign=Gtk.Align.START)
        tbox.append(ctl)
        self.toolbar.add_top_bar(Gtk.WindowHandle(child=tbox))

        # 2. navigation row
        nav = Gtk.Box(spacing=4, css_classes=["hbf-navbar"])

        def navbtn(icon, action, tip):
            b = Gtk.Button(icon_name=icon, action_name=action, tooltip_text=tip,
                           css_classes=["hbf-nav"])
            nav.append(b)
            return b
        navbtn("go-previous-symbolic", "win.back", "Back (Alt+Left)")
        navbtn("go-next-symbolic", "win.forward", "Forward (Alt+Right)")
        navbtn("go-up-symbolic", "win.up", "Up (Alt+Up)")
        navbtn("view-refresh-symbolic", "win.refresh", "Refresh (F5)")
        self.address = AddressBar(self)
        self.address.set_margin_start(6)
        nav.append(self.address)
        self.search = Gtk.SearchEntry(placeholder_text="Search", width_request=260,
                                      css_classes=["hb-search", "hbf-search"], search_delay=400)
        self.search.connect("search-changed", self._on_search_changed)
        self.search.connect("activate", self._on_search_changed)
        self.search.connect("stop-search", lambda *_: self._cancel_search())
        nav.append(self.search)
        self.toolbar.add_top_bar(nav)

        # 3. command bar
        from .taskmgr_ui import menu_tool_button, tool_button
        cmd = Gtk.Box(spacing=2, css_classes=["hbf-cmdbar"])
        newb = menu_tool_button("list-add-symbolic", "New", fm.new_menu())
        newb.add_css_class("hbf-new")
        newb.set_tooltip_text("Create a new item")
        cmd.append(newb)
        cmd.append(Gtk.Box(css_classes=["hbf-vsep"]))
        for icon, action, tip in (("edit-cut-symbolic", "win.cut", "Cut (Ctrl+X)"),
                                  ("edit-copy-symbolic", "win.copy", "Copy (Ctrl+C)"),
                                  ("edit-paste-symbolic", "win.paste", "Paste (Ctrl+V)"),
                                  ("document-edit-symbolic", "win.rename", "Rename (F2)"),
                                  ("user-trash-symbolic", "win.delete", "Delete (Delete)")):
            cmd.append(Gtk.Button(icon_name=icon, action_name=action, tooltip_text=tip,
                                  css_classes=["hb-tool"]))
        self.restore_btn = tool_button("edit-undo-symbolic", "Restore")
        self.restore_btn.set_action_name("win.restore")
        cmd.append(self.restore_btn)
        self.empty_btn = tool_button("user-trash-full-symbolic", "Empty trash")
        self.empty_btn.set_action_name("win.empty-trash")
        cmd.append(self.empty_btn)
        cmd.append(Gtk.Box(css_classes=["hbf-vsep"]))
        self.sort_btn = menu_tool_button("view-sort-descending-symbolic", "Sort", fm.sort_menu())
        cmd.append(self.sort_btn)
        self.view_btn = menu_tool_button("view-grid-symbolic", "View", fm.view_menu())
        cmd.append(self.view_btn)
        cmd.append(Gtk.Box(css_classes=["hbf-vsep"]))
        self.more_btn = Gtk.MenuButton(icon_name="view-more-symbolic", css_classes=["hb-tool"],
                                       menu_model=fm.more_menu(), tooltip_text="See more")
        self.more_btn.set_has_frame(False)
        cmd.append(self.more_btn)
        cmd.append(Gtk.Box(hexpand=True))
        det_box = Gtk.Box(spacing=8)
        det_box.append(Gtk.Image(icon_name="view-dual-symbolic", pixel_size=16))
        det_box.append(Gtk.Label(label="Details"))
        self.details_btn = Gtk.ToggleButton(child=det_box, css_classes=["hb-tool"],
                                            action_name="win.details-pane",
                                            tooltip_text="Show the details pane")
        cmd.append(self.details_btn)
        self.toolbar.add_top_bar(cmd)

        # body: sidebar | tabs | details pane
        from .files_pages import DetailsPane
        self.sidebar = fsb.Sidebar(self)
        self.sidebar.connect("open", lambda _s, loc, nt: self.open_location(loc, nt))
        self.details = DetailsPane(self)
        self.details.set_visible(bool(self.settings.get("details_pane")))
        body = Gtk.Box(css_classes=["hbf-body"])
        body.append(self.sidebar)
        body.append(self.tab_view)
        body.append(self.details)
        self.set_body(body)

        # status bar with the operations area
        status = Gtk.Box(spacing=12, css_classes=["hbf-status"])
        self.status_label = Gtk.Label(xalign=0, hexpand=True)
        status.append(self.status_label)
        self.ops_box = Gtk.Box(spacing=8, visible=False)
        self.ops_label = Gtk.Label(ellipsize=3, max_width_chars=40)
        self.ops_bar = Gtk.ProgressBar(width_request=160, valign=Gtk.Align.CENTER)
        cancel = Gtk.Button(icon_name="process-stop-symbolic", css_classes=["flat"],
                            tooltip_text="Cancel all operations", valign=Gtk.Align.CENTER)
        cancel.connect("clicked", lambda *_: self.ops.cancel_all())
        for w in (self.ops_label, self.ops_bar, cancel):
            self.ops_box.append(w)
        status.append(self.ops_box)
        self.toolbar.add_bottom_bar(status)

    # ------------------------------------------------------------ tabs
    def tab(self):
        page = self.tab_view.get_selected_page()
        return page.get_child() if page is not None else None

    def tabs(self):
        return [self.tab_view.get_nth_page(i).get_child()
                for i in range(self.tab_view.get_n_pages())]

    def new_tab(self, location=None, select=True):
        tab = Tab(self, location or fc.HOME_PAGE)
        page = self.tab_view.append(tab)
        tab.page = page
        tab.connect("changed", self._on_tab_changed)
        tab.view.cv.get_sorter().connect("changed", lambda *_: self._on_header_sort(tab))
        self._sync_page(tab)
        if select:
            self.tab_view.set_selected_page(page)
            GLib.idle_add(lambda: (tab.view.focus_view() if self.tab() is tab
                                   and tab.stack.get_visible_child_name() == "view" else None,
                                   False)[1])
        return tab

    def close_tab(self, tab=None):
        tab = tab or self.tab()
        if tab is not None:
            self.tab_view.close_page(tab.page)

    def _on_close_page(self, view, page):
        tab = page.get_child()
        tab.view.stop_search()
        view.close_page_finish(page, True)
        if view.get_n_pages() == 0:
            GLib.idle_add(lambda: (self.close(), False)[1])
        return True

    def _sync_page(self, tab):
        if tab.page is None:
            return
        tab.page.set_title(tab.title())
        tab.page.set_icon(Gio.ThemedIcon.new(tab.icon()))
        tab.page.set_tooltip(tab.folder_path() or tab.title())

    def _on_tab_changed(self, tab):
        self._sync_page(tab)
        if tab is self.tab():
            self._sync_window()

    def _on_switch(self):
        self._sync_window()

    def _sync_window(self):
        tab = self.tab()
        if tab is None:
            return
        self.set_title("%s - %s" % (tab.title(), APP_NAME))
        self.address.set_tab(tab)
        self._search_guard = True
        if self.search.get_text() != (tab.search_query or ""):
            self.search.set_text(tab.search_query or "")
        self._search_guard = False
        place = tab.title() if not tab.search_query else fc.location_title(tab.location)
        if tab.location == fc.THIS_PC or tab.location == fc.HOME_PAGE:
            place = "My files"
        self.search.set_placeholder_text("Search %s" % place)
        self.sidebar.sync(tab.location)
        self.update_actions()
        self._update_status()
        self._queue_details()

    # ------------------------------------------------------------ opening
    def open_location(self, location, new_tab=False):
        if not location:
            return
        if isinstance(location, str) and location.startswith("/"):
            location = fc.location_for(location)
        elif location == "trash":
            location = fc.TRASH_URI
        tab = self.tab()
        if new_tab or tab is None:
            self.new_tab(location, select=tab is None or not new_tab or True)
        else:
            tab.navigate(location)
            if tab.stack.get_visible_child_name() == "view":
                tab.view.focus_view()

    def open_arg(self, location, select_path=None, new_tab=False):
        if new_tab or self.tab() is None:
            tab = self.new_tab(location)
        else:
            tab = self.tab()
            tab.navigate(location)
        if select_path:
            tab.view.select_paths([select_path])

    def open_typed(self, text, cwd=None):
        """Address bar input; returns True when we went somewhere."""
        loc = fc.parse_location(text, cwd)
        if loc is None:
            return False
        if loc.startswith("/"):
            if not os.path.lexists(loc):
                self.notify_toast("Harbor Files can't find “%s”. Check the spelling and try "
                                  "again." % text.strip(), timeout=5)
                return False
            if not os.path.isdir(loc):
                self.open_arg(fc.location_for(os.path.dirname(loc)), loc)
                return True
        self.open_location(fc.location_for(loc))
        return True

    def open_uri(self, uri):
        """Open something from Recent (folder -> navigate, file -> app)."""
        g = Gio.File.new_for_uri(uri)

        def done(f, res):
            try:
                info = f.query_info_finish(res)
            except GLib.Error as e:
                self.notify_toast("Can't open this item: " + e.message, timeout=5)
                return
            if info.get_file_type() == Gio.FileType.DIRECTORY:
                self.open_location(uri)
            else:
                self.open_file(f, info.get_content_type() or "")
        g.query_info_async("standard::type,standard::content-type", Gio.FileQueryInfoFlags.NONE,
                           GLib.PRIORITY_DEFAULT, None, done)

    def open_file(self, gfile, content_type, app=None):
        name = gfile.get_basename() or gfile.get_uri()
        if is_executable_type(content_type):
            self.notify_toast("“%s” is a program. Harbor Files doesn't run programs; "
                              "start it from a terminal if you trust it." % name, timeout=6)
            return False
        if app is None and content_type and \
                Gio.AppInfo.get_default_for_type(content_type, False) is None:
            self.choose_app(gfile, content_type)
            return False
        return self.launch(gfile.get_uri(), app)

    def choose_app(self, gfile, content_type):
        from .files_pages import AppChooser
        AppChooser(self, gfile, content_type).present(self)

    def launch(self, uri, app=None):
        """The one place that starts other apps (dry runs only log it)."""
        if common.DRY_RUN:
            common.DRY_RUN_LOG.append(["open-with", app.get_id(), uri] if app else ["open", uri])
            print("[dry-run] open", uri, file=sys.stderr)
            return True
        ctx = self.get_display().get_app_launch_context()

        def done(src, res):
            try:
                if app is not None:
                    app.launch_uris_finish(res)
                else:
                    Gio.AppInfo.launch_default_for_uri_finish(res)
            except GLib.Error as e:
                self.notify_toast("Can't open this file: " + e.message, timeout=5)
                return
            try:
                Gtk.RecentManager.get_default().add_item(uri)
            except GLib.Error:
                pass
        if app is not None:
            app.launch_uris_async([uri], ctx, None, done)
        else:
            Gio.AppInfo.launch_default_for_uri_async(uri, ctx, None, done)
        return True

    def set_default_app(self, app, content_type):
        if common.DRY_RUN:
            common.DRY_RUN_LOG.append(["set-default", app.get_id(), content_type])
            return
        try:
            app.set_as_default_for_type(content_type)
        except GLib.Error as e:
            self.notify_toast("Can't change the default app: " + e.message)

    def open_items(self, tab, items, new_tab=False):
        if not items or tab.kind() == "trash":
            if items:
                self.notify_toast("Restore this item to open it")
            return
        dirs = [i for i in items if i.is_dir]
        files = [i for i in items if not i.is_dir]
        if len(dirs) == 1 and not files and not new_tab:
            tab.navigate(folder_location_of(dirs[0]))
            tab.view.focus_view()
            return
        for d in dirs:
            self.new_tab(folder_location_of(d), select=False)

        def go():
            for f in files:
                self.open_file(f.gfile, f.content_type)
        if len(files) > OPEN_CONFIRM_OVER:
            from .taskmgr_ui import confirm
            confirm(self, "Open %d files?" % len(files),
                    "Each file opens in its default app.", "Open all", go, destructive=False)
        else:
            go()

    def on_item_activated(self, tab, item):
        self.open_items(tab, [item])

    def on_middle_click(self, tab, item):
        if item is not None and item.is_dir and tab.kind() != "trash":
            self.new_tab(folder_location_of(item), select=False)

    # ------------------------------------------------------------ view callbacks
    def on_selection_changed(self, tab):
        if tab is self.tab():
            self.update_actions()
            self._update_status()
            self._queue_details()

    def on_contents_changed(self, tab):
        if tab is self.tab():
            self.update_actions()
            self._update_status()

    def on_drop(self, tab, files, dest, move):
        paths = [f.get_path() for f in files if f.get_path()]
        if not paths or dest is None or not dest.get_path():
            self.notify_toast("Can't drop these items here")
            return
        self.ops.transfer(paths, dest.get_path(), move=move,
                          on_done=lambda job: tab.view.select_paths(job.copied)
                          if dest.equal(tab.view.gfile or dest) else None)

    def on_context_menu(self, tab, x, y, item):
        self.update_actions()
        kind = tab.kind()
        if item is None:
            path = tab.folder_path()
            pinned = bool(path) and fsb.is_pinned(self.settings, path)
            m = fm.background_menu(kind, self.ops.can_paste(), tab.location, pinned)
        else:
            items = tab.view.selected_items() or [item]
            one = items[0]
            pinned = len(items) == 1 and one.is_dir and bool(one.path) \
                and fsb.is_pinned(self.settings, one.path)
            m = fm.item_menu(kind, sel_dicts(items), self.ops.can_paste(), pinned)
        self.popup_menu(tab.view, x, y, m)

    def _on_header_sort(self, tab):
        if tab is not self.tab():
            return
        field, desc = tab.view.current_sort()
        if (field, desc) != (self.settings.get("sort"), bool(self.settings.get("descending"))):
            self.settings.data["sort"] = field
            self.settings.set("descending", desc)
            self._set_state("sort", GLib.Variant("s", field))
            self._set_state("sort-order", GLib.Variant("s", "desc" if desc else "asc"))
            for t in self.tabs():
                if t is not tab:
                    t.view.set_sort(field, desc)

    def popup_menu(self, widget, x, y, menu):
        if self._popover is not None:
            self._popover.popdown()
        pop = Gtk.PopoverMenu.new_from_model(menu)
        pop.set_has_arrow(False)
        pop.set_halign(Gtk.Align.START)
        pop.set_parent(widget)
        rect = Gdk.Rectangle()
        rect.x, rect.y, rect.width, rect.height = int(x), int(y), 1, 1
        pop.set_pointing_to(rect)

        def closed(p):
            def drop():
                if p.get_parent() is not None:
                    p.unparent()
                if self._popover is p:
                    self._popover = None
                return False
            GLib.idle_add(drop)
        pop.connect("closed", closed)
        self._popover = pop
        pop.popup()
        return pop

    # ------------------------------------------------------------ status / details
    def _update_status(self):
        tab = self.tab()
        if tab is None:
            return
        kind = tab.kind()
        if kind in ("home", "thispc"):
            self.status_label.set_label("")
            return
        sel = tab.view.selected_items()
        n = len(tab.view.items())
        text = fc.status_text(n, len(sel), selection_size(sel))
        if tab.view.loading and tab.view.mode == "search":
            text += "  |  Searching…"
        self.status_label.set_label(text)

    def _queue_details(self):
        if not self.details.get_visible() or self._details_id:
            return

        def run():
            self._details_id = 0
            if self.details.get_visible():
                self.details.update(self.tab())
            return False
        self._details_id = GLib.timeout_add(80, run)

    # ------------------------------------------------------------ actions
    def _act(self, name, cb, ptype=None):
        a = Gio.SimpleAction.new(name, GLib.VariantType.new(ptype) if ptype else None)
        if ptype:
            a.connect("activate", lambda _a, p: cb(p.unpack()))
        else:
            a.connect("activate", lambda *_: cb())
        self.add_action(a)
        return a

    def _stateful(self, name, state, cb):
        a = Gio.SimpleAction.new_stateful(
            name, None if state.get_type_string() == "b" else state.get_type(), state)
        if state.get_type_string() == "b":
            a.connect("activate", lambda act, _p: act.change_state(
                GLib.Variant("b", not act.get_state().unpack())))
        a.connect("change-state", lambda act, v: (act.set_state(v), cb(v.unpack())))
        self.add_action(a)
        return a

    def _set_state(self, name, value):
        a = self.lookup_action(name)
        if a is not None:
            a.set_state(value)

    def _install_actions(self):
        s = self.settings
        A = self._act
        A("back", lambda: self._with_tab(lambda t: t.back()))
        A("forward", lambda: self._with_tab(lambda t: t.forward()))
        A("up", lambda: self._with_tab(lambda t: t.up()))
        A("refresh", lambda: self._with_tab(lambda t: t.refresh()))
        A("new-tab", lambda: self.new_tab())
        A("close-tab", lambda: self.close_tab())
        A("new-window", lambda: self.app.new_window())
        A("edit-location", lambda: self.address.edit())
        A("focus-search", self._focus_search)
        A("new-folder", lambda: self.new_item("folder"))
        A("new-text", lambda: self.new_item("text"))
        A("new-empty", lambda: self.new_item("empty"))
        A("cut", lambda: self.clipboard_put(True))
        A("copy", lambda: self.clipboard_put(False))
        A("paste", self.paste)
        A("paste-into", self.paste_into)
        A("rename", self.rename)
        A("delete", lambda: self.delete(False))
        A("delete-permanent", lambda: self.delete(True))
        A("restore", self.restore)
        A("empty-trash", self.empty_trash)
        A("open", lambda: self._with_tab(lambda t: self.open_items(t, t.view.selected_items())))
        A("open-tab", lambda: self._with_tab(
            lambda t: self.open_items(t, t.view.selected_items(), new_tab=True)))
        A("open-location-of", self.open_location_of)
        A("open-with-app", self.open_with_app, "s")
        A("open-with-other", self.open_with_other)
        A("open-location", lambda loc: self.open_location(loc), "s")
        A("open-location-tab", lambda loc: self.open_location(loc, True), "s")
        A("open-uri", self.open_uri, "s")
        A("open-uri-location", self.open_uri_location, "s")
        A("forget-recent", self.forget_recent, "s")
        A("select-all", lambda: self._with_tab(lambda t: t.view.select_all()))
        A("select-none", lambda: self._with_tab(lambda t: t.view.select_none()))
        A("invert-selection", lambda: self._with_tab(lambda t: t.view.invert_selection()))
        A("pin-toggle", self.pin_toggle)
        A("pin-at", lambda loc: self.set_pinned(_path(loc), True), "s")
        A("unpin", lambda loc: self.set_pinned(_path(loc), False), "s")
        A("terminal", lambda: self._with_tab(lambda t: self.terminal_at(t.folder_path())))
        A("terminal-sel", self.terminal_sel)
        A("terminal-at", lambda loc: self.terminal_at(_path(loc)), "s")
        A("copy-path", self.copy_path)
        A("copy-path-at", lambda loc: self._copy_text(fc.quote_path(_path(loc) or loc)), "s")
        A("properties", self.properties)
        A("properties-at", lambda loc: self.show_properties([_gfile(loc)]), "s")
        A("compress", self.compress)
        A("extract", self.extract)
        A("undo", self.undo)
        A("about", lambda: common.about_dialog(self, APP_NAME, ICON,
                                               "A Windows 11 style file manager for GNOME"))
        A("sb-mount", self._sb_mount, "s")
        A("sb-eject", lambda key: self.sidebar.unmount(key, eject=True), "s")
        A("sb-unmount", lambda key: self.sidebar.unmount(key, eject=False), "s")
        view_key = "details" if s.get("view") == "details" else s.get("icon_size")
        self._stateful("view", GLib.Variant("s", view_key), self._set_view)
        self._stateful("sort", GLib.Variant("s", s.get("sort")), self._set_sort_field)
        self._stateful("sort-order", GLib.Variant("s", "desc" if s.get("descending") else "asc"),
                       self._set_sort_order)
        self._stateful("show-hidden", GLib.Variant("b", bool(s.get("show_hidden"))),
                       self._set_hidden)
        self._stateful("show-ext", GLib.Variant("b", bool(s.get("show_extensions"))),
                       self._set_ext)
        self._stateful("details-pane", GLib.Variant("b", bool(s.get("details_pane"))),
                       self._set_details)

    def _with_tab(self, fn):
        tab = self.tab()
        if tab is not None:
            return fn(tab)
        return None

    def _sel(self):
        tab = self.tab()
        if tab is None or tab.kind() in ("home", "thispc"):
            return tab, []
        return tab, tab.view.selected_items()

    def update_actions(self):
        if not hasattr(self, "ops"):
            return
        tab = self.tab()
        if tab is None:
            return
        kind = tab.kind()
        tab_, items = self._sel()
        listing = kind not in ("home", "thispc")
        st = fc.command_states(kind, sel_dicts(items), can_paste=self.ops.can_paste(),
                               writable=tab.writable,
                               n_items=len(tab.view.items()) if listing else 0)
        for name, key in ACTION_RULES.items():
            self._enable(name, st[key])
        one = items[0] if len(items) == 1 else None
        self._enable("paste-into", bool(one and one.is_dir and one.path and self.ops.can_paste()))
        self._enable("terminal", kind == "folder" and bool(tab.folder_path()))
        self._enable("terminal-sel", bool(one and one.is_dir and one.path))
        self._enable("properties", bool(items) or (kind in ("folder", "trash")))
        self._enable("open-location-of", kind == "search" and len(items) == 1)
        self._enable("back", tab.can_back())
        self._enable("forward", tab.can_forward())
        self._enable("up", tab.can_up())
        self._enable("undo", bool(self._undo))
        self._enable("view", listing)
        self._enable("sort", listing)
        self._enable("sort-order", listing)
        self.restore_btn.set_visible(kind == "trash")
        self.empty_btn.set_visible(kind == "trash")
        target = self._pin_target()
        pinned = bool(target) and fsb.is_pinned(self.settings, target)
        label = "Unpin from Quick access" if pinned else "Pin to Quick access"
        if getattr(self, "_pin_label", None) != label:
            self._pin_label = label
            self.more_btn.set_menu_model(fm.more_menu(label))

    def _enable(self, name, value):
        a = self.lookup_action(name)
        if a is not None:
            a.set_enabled(bool(value))

    # ------------------------------------------------------------ view settings
    def _set_view(self, key):
        view = "details" if key == "details" else "icons"
        size = key if key in ("large", "medium") else self.settings.get("icon_size")
        self.settings.data["icon_size"] = size
        self.settings.set("view", view)
        for t in self.tabs():
            t.view.set_view(view, size)

    def _set_sort_field(self, field):
        if field not in fc.SORT_FIELDS:
            return
        self.settings.set("sort", field)
        for t in self.tabs():
            t.view.set_sort(field, bool(self.settings.get("descending")))

    def _set_sort_order(self, order):
        desc = order == "desc"
        self.settings.set("descending", desc)
        for t in self.tabs():
            t.view.set_sort(self.settings.get("sort"), desc)

    def _set_hidden(self, value):
        self.settings.set("show_hidden", bool(value))
        for t in self.tabs():
            t.view.set_show_hidden(bool(value))
        self._update_status()

    def _set_ext(self, value):
        self.settings.set("show_extensions", bool(value))
        for t in self.tabs():
            t.view.set_show_extensions(bool(value))

    def _set_details(self, value):
        self.settings.set("details_pane", bool(value))
        self.details.set_visible(bool(value))
        self._queue_details()

    # ------------------------------------------------------------ commands
    def _focus_search(self):
        self.search.grab_focus()
        self.search.select_region(0, -1)

    def _on_search_changed(self, *_):
        if self._search_guard:
            return
        tab = self.tab()
        if tab is None:
            return
        q = self.search.get_text().strip()
        if q:
            if q != tab.search_query:
                tab.start_search(q)
        else:
            tab.end_search()

    def _cancel_search(self):
        tab = self.tab()
        self._search_guard = True
        self.search.set_text("")
        self._search_guard = False
        if tab is not None:
            tab.end_search()
            if tab.stack.get_visible_child_name() == "view":
                tab.view.focus_view()

    def new_item(self, kind):
        tab = self.tab()
        folder = tab.folder_path() if tab else None
        if not folder or tab.kind() != "folder":
            return
        try:
            path = fc.new_item(folder, kind)
        except OSError as e:
            self.notify_toast("Can't create the item: %s" % (e.strerror or e), timeout=5)
            return
        self.rename_when_visible(tab, path, kind == "folder")

    def rename_when_visible(self, tab, path, is_dir, tries=40):
        """Wait (up to ~4 s) for the new item to show up, then start renaming."""
        uri = Gio.File.new_for_path(path).get_uri()
        state = {"n": 0}

        def poll():
            state["n"] += 1
            item = tab.view.by_uri.get(uri)
            if item is None:
                return state["n"] < tries
            tab.view.select_paths([path])
            GLib.timeout_add(60, lambda: (self.start_rename(tab, item), False)[1])
            return False
        GLib.timeout_add(100, poll)

    def rename(self):
        tab, items = self._sel()
        if len(items) == 1:
            self.start_rename(tab, items[0])

    def start_rename(self, tab, item):
        if not item.path:
            return
        pos = tab.view.position_of(item)
        if pos >= 0:
            tab.view.scroll_to(pos)

        def done(name):
            old = item.name
            try:
                new = fc.rename_path(item.path, name)
            except ValueError as e:
                self.notify_toast(str(e), timeout=4)
                return False
            except GLib.Error as e:
                self.notify_toast("Can't rename: " + e.message, timeout=5)
                return False
            if new != item.path:
                self._undo.append(("rename", new, old))
                tab.view.select_paths([new])
                self.update_actions()
            return True

        def show():
            from .files_pages import RenamePopover
            widget = tab.view.item_widget(item)
            pop = RenamePopover(widget or tab.view, item.name, item.is_dir, done)
            if widget is None:
                rect = Gdk.Rectangle()
                rect.x, rect.y, rect.width, rect.height = 40, 40, 1, 1
                pop.set_pointing_to(rect)
            self._rename_popover = pop
            pop.popup()
            return False
        GLib.idle_add(show)

    def clipboard_put(self, cut):
        tab, items = self._sel()
        paths = [i.path for i in items if i.path]
        if paths and tab.kind() in ("folder", "search"):
            self.ops.set_clipboard(paths, cut)

    def paste(self):
        tab = self.tab()
        dest = tab.folder_path() if tab else None
        if dest and tab.kind() == "folder":
            self.ops.paste(dest, on_done=lambda job: tab.view.select_paths(job.copied))

    def paste_into(self):
        tab, items = self._sel()
        if len(items) == 1 and items[0].is_dir and items[0].path:
            self.ops.paste(items[0].path)

    def delete(self, permanent=False):
        from .taskmgr_ui import confirm
        tab, items = self._sel()
        if not items:
            return
        n = len(items)
        what = "“%s”" % items[0].name if n == 1 else fc.item_count_text(n)
        if tab.kind() == "trash":
            gfiles = [i.gfile for i in items]
            confirm(self, "Delete permanently?",
                    "%s will be deleted for good. You can't undo this." % what.capitalize()
                    if n > 1 else "%s will be deleted for good. You can't undo this." % what,
                    "Delete", lambda: self.ops.delete_gfiles(
                        gfiles, "Deleting %s" % fc.item_count_text(n)))
            return
        paths = [i.path for i in items if i.path]
        if not paths:
            return
        no_trash = [i for i in items if not i.can("trash")]
        if permanent or no_trash:
            body = "%s will be deleted for good. You can't undo this." % what
            if no_trash and not permanent:
                body = ("%s can't be moved to the trash on this drive. Delete permanently? "
                        "You can't undo this." % what)
            confirm(self, "Delete permanently?", body[0].upper() + body[1:], "Delete",
                    lambda: self.ops.delete_permanently(paths))
            return

        def done(job):
            gone = [p for p in paths if not os.path.lexists(p)]
            if gone:
                self._undo.append(("trash", gone, time.time()))
                self.update_actions()
            if not job.errors and not job.cancelled:
                toast = Adw.Toast(title="Moved %s to the trash" % fc.item_count_text(len(gone)),
                                  timeout=5, button_label="Undo", action_name="win.undo")
                self.toast.add_toast(toast)
        self.ops.trash(paths, on_done=done)

    def restore(self):
        tab, items = self._sel()
        if tab is None or tab.kind() != "trash" or not items:
            return
        self.ops.restore([(i.gfile, i.orig_path) for i in items])

    def empty_trash(self):
        from .taskmgr_ui import confirm

        def go():
            def work():
                out = []
                root = Gio.File.new_for_uri(fc.TRASH_URI)
                en = root.enumerate_children("standard::name", Gio.FileQueryInfoFlags.NONE, None)
                for info in en:
                    out.append(en.get_child(info))
                en.close(None)
                return out

            def done(res):
                if isinstance(res, Exception):
                    self.notify_toast("Can't read the trash: %s" % res, timeout=5)
                    return
                if not res:
                    self.notify_toast("The trash is already empty")
                    return
                self.ops.delete_gfiles(res, "Emptying the trash")
            fc.run_in_thread(work, done)
        confirm(self, "Empty the trash?",
                "All items in the trash will be deleted for good. You can't undo this.",
                "Empty trash", go)

    def undo(self):
        if not self._undo:
            return
        kind, a, b = self._undo.pop()
        self.update_actions()
        if kind == "rename":
            try:
                new = fc.rename_path(a, b)
                self.notify_toast("Renamed back to “%s”" % os.path.basename(new))
            except (ValueError, GLib.Error) as e:
                self.notify_toast("Can't undo: %s" % getattr(e, "message", e), timeout=5)
        elif kind == "trash":
            self._undo_trash(a, b)

    def _undo_trash(self, paths, since):
        want = set(paths)

        def work():
            found = []
            root = Gio.File.new_for_uri(fc.TRASH_URI)
            en = root.enumerate_children("standard::name,trash::orig-path,trash::deletion-date",
                                         Gio.FileQueryInfoFlags.NONE, None)
            best = {}
            for info in en:
                orig = info.get_attribute_byte_string("trash::orig-path")
                if orig not in want:
                    continue
                dd = info.get_deletion_date()
                ts = dd.to_unix() if dd else 0
                if ts + 120 < since:
                    continue
                if orig not in best or ts > best[orig][1]:
                    best[orig] = (en.get_child(info), ts)
            en.close(None)
            for orig, (g, _ts) in best.items():
                found.append((g, orig))
            return found

        def done(res):
            if isinstance(res, Exception) or not res:
                self.notify_toast("Can't find these items in the trash any more")
                return
            self.ops.restore(res)
        fc.run_in_thread(work, done)

    def open_location_of(self):
        tab, items = self._sel()
        if len(items) == 1 and items[0].path:
            p = items[0].path
            self.open_arg(fc.location_for(os.path.dirname(p)), p)

    def open_uri_location(self, uri):
        p = _path(uri)
        if p:
            self.open_arg(fc.location_for(os.path.dirname(p)), p)

    def forget_recent(self, uri):
        try:
            Gtk.RecentManager.get_default().remove_item(uri)
        except GLib.Error:
            pass
        for t in self.tabs():
            if t.home_page is not None:
                t.home_page.queue_refresh()

    def open_with_app(self, app_id):
        tab, items = self._sel()
        if len(items) != 1:
            return
        app = Gio.DesktopAppInfo.new(app_id)
        if app is None:
            self.notify_toast("That app is no longer installed")
            return
        self.open_file(items[0].gfile, items[0].content_type, app=app)

    def open_with_other(self):
        tab, items = self._sel()
        if len(items) == 1 and not items[0].is_dir:
            self.choose_app(items[0].gfile, items[0].content_type)

    def _pin_target(self):
        tab, items = self._sel()
        if tab is None:
            return None
        if len(items) == 1 and items[0].is_dir and items[0].path:
            return items[0].path
        if not items and tab.kind() == "folder":
            return tab.folder_path()
        return None

    def pin_toggle(self):
        path = self._pin_target()
        if path:
            self.set_pinned(path, not fsb.is_pinned(self.settings, path))

    def set_pinned(self, path, pinned):
        if not path:
            return
        if pinned:
            fsb.pin(self.settings, path)
        else:
            fsb.unpin(self.settings, path)
        for w in self.app.get_windows():
            if isinstance(w, FilesWindow):
                w.sidebar.rebuild()
                if w.tab():
                    w.sidebar.sync(w.tab().location)
                for t in w.tabs():
                    if t.home_page is not None:
                        t.home_page.queue_refresh()
                w.update_actions()

    def terminal_at(self, path):
        if not path:
            return
        if not open_terminal(path):
            self.notify_toast("No terminal app was found")

    def terminal_sel(self):
        tab, items = self._sel()
        if len(items) == 1 and items[0].is_dir and items[0].path:
            self.terminal_at(items[0].path)

    def _copy_text(self, text):
        if text:
            self.ops.set_text(text)
            self.notify_toast("Path copied")

    def copy_path(self):
        tab, items = self._sel()
        paths = [i.path for i in items if i.path]
        if not paths and tab is not None and not items:
            p = tab.folder_path()
            paths = [p] if p else []
        self._copy_text("\n".join(fc.quote_path(p) for p in paths))

    def properties(self):
        tab, items = self._sel()
        if items:
            self.show_properties([i.gfile for i in items])
        elif tab is not None and tab.kind() in ("folder", "trash"):
            self.show_properties([Gio.File.new_for_uri(tab.location)])

    def show_properties(self, gfiles):
        gfiles = [g for g in gfiles if g is not None]
        if not gfiles:
            return
        from .files_pages import PropertiesDialog
        return PropertiesDialog(self, gfiles)   # presents itself

    def compress(self):
        tab, items = self._sel()
        paths = [i.path for i in items if i.path]
        if not paths:
            return
        folder = os.path.dirname(os.path.normpath(paths[0]))
        dest = os.path.join(folder, fc.unique_name(fc.zip_name_for(paths), fc.exists_in(folder)))
        job = fc.Job()

        def done(j):
            if j.copied and not j.errors and tab.view.gfile is not None \
                    and tab.folder_path() == folder:
                tab.view.select_paths(j.copied)
        self.ops._start("Compressing %s" % fc.item_count_text(len(paths)),
                        lambda j: fc.compress_zip(j, paths, dest), job, done)

    def extract(self):
        tab, items = self._sel()
        if len(items) != 1 or not items[0].path:
            return
        archive = items[0].path
        folder = os.path.dirname(archive)
        job = fc.Job()

        def done(j):
            if j.copied and tab.folder_path() == folder:
                tab.view.select_paths(j.copied)
        self.ops._start("Extracting “%s”" % os.path.basename(archive),
                        lambda j: fc.extract_archive(j, archive, folder), job, done)

    def _sb_mount(self, key):
        t = self.sidebar.targets.get(key)
        if isinstance(t, tuple) and t and t[0] == "volume":
            self.sidebar.mount_volume(t[1])

    def refresh_after_op(self):
        self.update_actions()
        self.sidebar.update_trash()

    # ------------------------------------------------------------ show_page (scripting / tests)
    def show_page(self, key):
        kind, arg, extra = parse_page_key(key)
        if kind in ("home", "thispc", "trash", "network"):
            self.open_location(fc.location_for(kind) if kind != "network" else fc.NETWORK_URI)
        elif kind == "newtab":
            self.new_tab()
        elif kind == "folder" and arg:
            self.open_location(fc.location_for(os.path.expanduser(arg)))
        elif kind == "search":
            if arg:
                self.open_location(fc.location_for(os.path.expanduser(arg)))
            tab = self.tab()
            self._search_guard = True
            self.search.set_text(extra or "")
            self._search_guard = False
            if extra:
                tab.start_search(extra)
        elif kind == "properties" and arg:
            return self.show_properties([_gfile(os.path.expanduser(arg))])
        elif kind == "details":
            self.lookup_action("details-pane").change_state(GLib.Variant("b", True))
        elif kind == "menu":
            return self.show_menu(arg)
        else:
            raise ValueError("unknown page: %s" % key)
        return None

    def show_menu(self, which=None):
        """Pop up a context menu (for screenshots): which = item | background."""
        tab = self.tab()
        if tab is None:
            return None
        items = tab.view.items()
        w = tab.view.get_width() or 600
        if which != "background" and items:
            if not tab.view.selected_items():
                tab.view.select_item(items[0])
            return self.on_context_menu(tab, 160, 70, tab.view.selected_items()[0]) or self._popover
        self.on_context_menu(tab, min(w - 40, 420), 220, None)
        return self._popover

    # ------------------------------------------------------------ keyboard
    def _install_keys(self):
        key = Gtk.EventControllerKey(propagation_phase=Gtk.PropagationPhase.CAPTURE)
        key.connect("key-pressed", self._on_key)
        self.add_controller(key)
        mouse = Gtk.GestureClick(button=0, propagation_phase=Gtk.PropagationPhase.CAPTURE)
        mouse.connect("pressed", self._on_mouse)
        self.add_controller(mouse)

    def _on_mouse(self, gesture, _n, _x, _y):
        b = gesture.get_current_button()
        if b == 8:
            self.activate_action("win.back", None)
        elif b == 9:
            self.activate_action("win.forward", None)

    def _focus_in(self, ancestor_type=None, widget=None):
        f = self.get_focus()
        while f is not None:
            if widget is not None and f is widget:
                return True
            if ancestor_type is not None and isinstance(f, ancestor_type):
                return True
            f = f.get_parent()
        return False

    def _on_key(self, _c, keyval, _code, state):
        focus = self.get_focus()
        if isinstance(focus, Gtk.Editable) or self._focus_in(Adw.Dialog) \
                or self._focus_in(widget=self.sidebar):
            return False
        tab = self.tab()
        if tab is None:
            return False
        mods = state & (Gdk.ModifierType.CONTROL_MASK | Gdk.ModifierType.SHIFT_MASK
                        | Gdk.ModifierType.ALT_MASK)
        ctrl = mods == Gdk.ModifierType.CONTROL_MASK
        shift = mods == Gdk.ModifierType.SHIFT_MASK
        in_view = self._focus_in(widget=tab.view)
        k = keyval
        if k in (Gdk.KEY_Return, Gdk.KEY_KP_Enter, Gdk.KEY_ISO_Enter) and not mods and in_view:
            self.activate_action("win.open", None)
            return True
        if k == Gdk.KEY_BackSpace and not mods:
            tab.up()
            return True
        if k == Gdk.KEY_F2 and not mods:
            self.rename()
            return True
        if k in (Gdk.KEY_Delete, Gdk.KEY_KP_Delete):
            if not mods:
                self.delete(False)
                return True
            if shift:
                self.delete(True)
                return True
        if k == Gdk.KEY_Escape and not mods and tab.search_query:
            self._cancel_search()
            return True
        if ctrl:
            low = Gdk.keyval_to_lower(k)
            if low == Gdk.KEY_c:
                self.clipboard_put(False)
                return True
            if low == Gdk.KEY_x:
                self.clipboard_put(True)
                return True
            if low == Gdk.KEY_v:
                self.paste()
                return True
            if low == Gdk.KEY_a:
                if tab.kind() not in ("home", "thispc"):
                    tab.view.select_all()
                return True
            if low == Gdk.KEY_z:
                self.undo()
                return True
        if not mods or shift:
            ch = Gdk.keyval_to_unicode(k)
            if ch and chr(ch).isprintable() and not chr(ch).isspace() and in_view:
                return self._typeahead_key(tab, chr(ch))
        return False

    def _typeahead_key(self, tab, ch):
        prefix, last = self._typeahead
        now = time.monotonic()
        prefix = (prefix if now - last < 1.0 else "") + ch.casefold()
        self._typeahead = (prefix, now)
        items = tab.view.items()
        cur = tab.view.selected_items()
        start = tab.view.position_of(cur[0]) if cur else -1
        order = items[start:] + items[:start] if start >= 0 and len(prefix) > 1 \
            else items[start + 1:] + items[:start + 1]
        for it in order:
            if tab.view.label_for(it).casefold().startswith(prefix):
                tab.view.select_item(it)
                tab.view.scroll_to(tab.view.position_of(it))
                return True
        return True

    # ------------------------------------------------------------ closing
    def _on_close_request(self, *_):
        s = self.settings
        if not self.is_maximized():
            w, h = self.get_default_size()
            if w > 300 and h > 200:
                s.data["width"], s.data["height"] = w, h
        s.set("maximized", bool(self.is_maximized()))
        if self.ops.jobs:
            self.ops.cancel_all()
        for t in self.tabs():
            t.view.stop_search()
        return False


def _path(loc):
    if not loc:
        return None
    if loc.startswith("/"):
        return loc
    if loc in (fc.HOME_PAGE,):
        return fc.home_dir()
    return Gio.File.new_for_uri(loc).get_path()


def _gfile(loc):
    if not loc:
        return None
    if loc == fc.HOME_PAGE:
        return Gio.File.new_for_path(fc.home_dir())
    if loc.startswith("/"):
        return Gio.File.new_for_path(loc)
    return Gio.File.new_for_uri(loc)


# ---------------------------------------------------------------------- app

class App(Adw.Application):
    def __init__(self):
        super().__init__(application_id=APP_ID,
                         flags=Gio.ApplicationFlags.HANDLES_COMMAND_LINE)
        self.add_main_option("new-window", 0, GLib.OptionFlags.NONE, GLib.OptionArg.NONE,
                             "Open a new window", None)
        self.settings = fc.Settings()

    def do_startup(self):
        Adw.Application.do_startup(self)
        apply_theme(CSS)
        here = os.path.dirname(os.path.realpath(__file__))
        theme = Gtk.IconTheme.get_for_display(Gdk.Display.get_default())
        for d in (os.path.join(here, "..", "..", "data", "icons"), "/usr/share/harbor/icons"):
            if os.path.isdir(d):
                theme.add_search_path(d)
        Gtk.Window.set_default_icon_name(ICON)
        for action, keys in (
                ("win.new-tab", ["<Control>t"]), ("win.close-tab", ["<Control>w"]),
                ("win.new-window", ["<Control>n"]),
                ("win.edit-location", ["<Control>l", "<Alt>d"]),
                ("win.focus-search", ["<Control>f", "<Control>e"]),
                ("win.refresh", ["F5", "<Control>r"]),
                ("win.back", ["<Alt>Left"]), ("win.forward", ["<Alt>Right"]),
                ("win.up", ["<Alt>Up"]),
                ("win.view('large')", ["<Control><Shift>2", "<Control>1"]),
                ("win.view('medium')", ["<Control><Shift>3", "<Control>2"]),
                ("win.view('details')", ["<Control><Shift>6", "<Control>3"]),
                ("win.show-hidden", ["<Control>h"]),
                ("win.new-folder", ["<Control><Shift>n"]),
                ("win.properties", ["<Alt>Return"]),
                ("app.quit", ["<Control>q"])):
            self.set_accels_for_action(action, keys)
        quit_ = Gio.SimpleAction.new("quit", None)
        quit_.connect("activate", lambda *_: [w.close() for w in self.get_windows()])
        self.add_action(quit_)

    def new_window(self, locations=None):
        win = FilesWindow(self, locations)
        win.present()
        return win

    def do_command_line(self, cmdline):
        opts = cmdline.get_options_dict()
        force_new = opts.contains("new-window")
        cwd = cmdline.get_cwd()
        locs = []
        for a in cmdline.get_arguments()[1:]:
            if a.startswith("--"):
                if a == "--new-window":
                    force_new = True
                continue
            r = arg_to_location(a, cwd)
            if r:
                locs.append(r)
        win = None if force_new else self.get_active_window()
        if win is None:
            self.new_window(locs or None)
        else:
            for loc, sel in locs:
                win.open_arg(loc, sel, new_tab=True)
            win.present()
        return 0

    def do_activate(self):
        win = self.get_active_window()
        if win is None:
            self.new_window()
        else:
            win.present()


def main(argv=None):
    argv = list(argv if argv is not None else sys.argv)
    return App().run(argv)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
