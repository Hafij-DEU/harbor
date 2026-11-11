"""Harbor Task Manager: a Windows 11 style process and performance monitor."""

import os
import signal
import sys

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gio, GLib, Gtk  # noqa: E402

from . import common  # noqa: E402
from . import taskmgr_proc as tp  # noqa: E402
from . import taskmgr_store as ts  # noqa: E402
from . import taskmgr_sweep as ms  # noqa: E402
from .common import (HarborWindow, NavList, apply_theme, have, label, run_cmd,  # noqa: E402
                     spawn)
from .taskmgr_model import (MB, AppLookup, ProcModels, fmt_cpu, fmt_disk,  # noqa: E402
                            fmt_mem)
from .taskmgr_ui import (CSS, Col, Page, Table, cmp, confirm, copy_text,  # noqa: E402
                         info_dialog, key_sorter, menu_tool_button, tool_button)

APP_ID = "io.github.hafij_deu.Harbor.TaskManager"
ICON = APP_ID

NAV = [
    ("processes", "Processes", "view-grid-symbolic"),
    ("performance", "Performance", "org.gnome.SystemMonitor-symbolic"),
    ("sweep", "Memory Sweep", "edit-clear-all-symbolic"),
    ("history", "App history", "document-open-recent-symbolic"),
    ("startup", "Startup apps", "power-profile-performance-symbolic"),
    ("users", "Users", "system-users-symbolic"),
    ("details", "Details", "view-list-bullet-symbolic"),
    ("services", "Services", "applications-system-symbolic"),
]
PAGE_KEYS = [k for k, _l, _i in NAV] + ["settings"]


def config_path(name):
    return os.path.join(GLib.get_user_config_dir(), "harbor", name)


def data_path(name):
    return os.path.join(GLib.get_user_data_dir(), "harbor", name)


def net_cell(item):
    """Closest Linux equivalent of the per-process network column: active
    connections (sockets of own processes; others are not readable)."""
    procs = [item.proc] if item.proc is not None else item.procs
    vals = [p.conns for p in procs if p.conns is not None]
    if not vals:
        return "—"
    n = sum(vals)
    if n == 0:
        return "0 conn."
    return "%d conn." % n


def mbps(rate):
    v = rate * 8 / 1e6
    if v < 0.05:
        return "0 Mbps"
    if v < 10:
        return "%.1f Mbps" % v
    return "%.0f Mbps" % v


# ------------------------------------------------------------------ process pages

class ProcTablePage(Page):
    """Shared code of Processes / Users / Details: actions on the selection."""

    def selected(self):
        return self.table.selected_item()

    def selected_procs(self):
        return self.app.models.procs_of(self.selected())

    def sync_actions(self):
        pass


class ProcessesPage(ProcTablePage):
    key = "processes"
    title = "Processes"

    def __init__(self, win):
        super().__init__(win)
        m = self.app.models
        self.sort_key = "name"
        self.sort_desc = False
        self.sorter = Gtk.CustomSorter.new(self._compare)
        self.tree = Gtk.TreeListModel.new(
            Gtk.FilterListModel(model=m.root, filter=m.tree_filter), False, False,
            m.create_tree_child)
        sort = Gtk.SortListModel(model=self.tree, sorter=Gtk.TreeListRowSorter.new(self.sorter))
        sel = Gtk.SingleSelection(model=sort, autoselect=False, can_unselect=True)
        mem_total = lambda: max(1, self.app.sys.mem.get("MemTotal", 1))  # noqa: E731
        cols = [
            Col("name", "Name", lambda i: i.label, expand=True, kind="name",
                sort=key_sorter(lambda i: 0)),
            Col("status", "Status", lambda i: i.status, width=140, kind="status",
                sort=key_sorter(lambda i: 0)),
            Col("cpu", "CPU", lambda i: fmt_cpu(i.cpu), width=84, kind="num",
                heat=lambda i: i.cpu / 100 * 1.6, sort=key_sorter(lambda i: 0)),
            Col("mem", "Memory", lambda i: fmt_mem(i.mem), width=104, kind="num",
                heat=lambda i: i.mem / mem_total() * 3, sort=key_sorter(lambda i: 0)),
            Col("disk", "Disk", lambda i: fmt_disk(i.disk), width=96, kind="num",
                heat=lambda i: i.disk / (40 * MB), sort=key_sorter(lambda i: 0)),
            Col("net", "Network", net_cell, width=100, kind="num",
                dim=lambda i: net_cell(i) == "—", sort=key_sorter(lambda i: 0)),
        ]
        self.table = Table(sel, cols, on_menu=self._menu, on_activate=self._activate,
                           base_depth=1)
        cvs = self.table.cv.get_sorter()
        cvs.connect("changed", self._sort_changed)
        self._sorting = False
        sel.connect("notify::selected-item", lambda *_: self.sync_actions())
        self.box.append(self.table.scroller)
        a = self.actions
        a.append(tool_button("tab-new-symbolic", "Run new task", win.run_new_task))
        self.end_btn = tool_button("process-stop-symbolic", "End task", win.end_selected)
        a.append(self.end_btn)
        self.eff_btn = tool_button("emoji-nature-symbolic", "Efficiency mode",
                                   win.toggle_efficiency)
        a.append(self.eff_btn)
        view = Gio.Menu()
        view.append("Group by type", "win.group-by-type")
        cols_menu = Gio.Menu()
        for key, title in (("status", "Status"), ("cpu", "CPU"), ("mem", "Memory"),
                           ("disk", "Disk"), ("net", "Network")):
            cols_menu.append(title, "win.col-" + key)
        view.append_section("Columns", cols_menu)
        exp = Gio.Menu()
        exp.append("Expand all", "win.expand-all")
        exp.append("Collapse all", "win.collapse-all")
        view.append_section(None, exp)
        a.append(menu_tool_button("view-reveal-symbolic", "View", view))
        self.sync_actions()
        GLib.idle_add(self._init_header)

    def _init_header(self):
        self.update_header()
        return False

    # sorting: groups stay in their fixed order, everything else follows the
    # clicked column; numeric columns start descending like Windows
    def _compare(self, a, b, *_):
        if a.kind == "group" and b.kind == "group":
            return cmp(a.order, b.order)
        k = self.sort_key
        if k == "name":
            r = cmp(a.label.lower(), b.label.lower())
        elif k == "status":
            r = cmp(a.status, b.status)
        elif k == "net":
            r = cmp(_conns(a), _conns(b))
        else:
            r = cmp(getattr(a, k), getattr(b, k))
        if r == 0:
            r = cmp(a.label.lower(), b.label.lower())
            return r
        return -r if self.sort_desc else r

    def _sort_changed(self, sorter, _change):
        if self._sorting:
            return
        col = sorter.get_primary_sort_column()
        order = sorter.get_primary_sort_order()
        key = "name"
        for k, c in self.table.columns.items():
            if c == col:
                key = k
        if col is None:
            key = "name"
            order = Gtk.SortType.ASCENDING
        numeric = key in ("cpu", "mem", "disk", "net")
        if numeric and key != self.sort_key and order == Gtk.SortType.ASCENDING:
            self._sorting = True
            self.table.cv.sort_by_column(col, Gtk.SortType.DESCENDING)
            self._sorting = False
            order = Gtk.SortType.DESCENDING
        self.sort_key = key
        self.sort_desc = order == Gtk.SortType.DESCENDING
        self.sorter.changed(Gtk.SorterChange.DIFFERENT)

    def update_header(self):
        s = self.app.sys
        t = self.table
        t.set_header("name", None, "Name", right=False)
        t.set_header("status", None, "Status", right=False)
        t.set_header("cpu", "%d%%" % round(s.cpu), "CPU")
        t.set_header("mem", "%d%%" % round(s.mem_used_fraction() * 100), "Memory")
        t.set_header("disk", "%d%%" % round(s.disk_total_active()), "Disk")
        t.set_header("net", mbps(s.net_total()), "Network")

    def on_tick(self):
        self.update_header()
        # groups are always open
        n = self.tree.get_model().get_n_items()
        if self.app.models.group_by_type:
            for i in range(n):
                row = self.tree.get_child_row(i)
                if row is not None and not row.get_expanded():
                    row.set_expanded(True)
        if self.sort_key != "name":
            self.sorter.changed(Gtk.SorterChange.DIFFERENT)
        self.sync_actions()

    def on_show(self, sub=None):
        self.on_tick()

    def on_search(self, text):
        self.app.models.set_query(text)
        if text:
            self.expand_all(True)
        self.on_tick()

    def expand_all(self, on):
        model = self.table.selection.get_model()
        i = 0
        while i < model.get_n_items():
            row = model.get_item(i)
            item = row.get_item()
            if item.kind in ("app", "unit"):
                row.set_expanded(on)
            i += 1

    def sync_actions(self):
        item = self.selected()
        procs = self.app.models.procs_of(item)
        self.end_btn.set_sensitive(bool(procs))
        self.eff_btn.set_sensitive(bool(procs))
        on = bool(procs) and all(p.efficiency for p in procs)
        self.eff_btn.text_label.set_label("Efficiency mode" + (" ✓" if on else ""))

    def _activate(self, pos):
        row = self.table.selection.get_item(pos)
        if isinstance(row, Gtk.TreeListRow) and row.is_expandable():
            row.set_expanded(not row.get_expanded())

    def _menu(self, cv, x, y):
        item = self.selected()
        if item is None or item.kind == "group":
            return
        m = Gio.Menu()
        if item.kind in ("app", "unit"):
            row = self.table.selected_row()
            m.append("Collapse" if row and row.get_expanded() else "Expand", "win.tm-expand")
        s1 = Gio.Menu()
        s1.append("End task", "win.tm-end")
        s1.append("Efficiency mode", "win.tm-efficiency")
        m.append_section(None, s1)
        s2 = Gio.Menu()
        s2.append("Open file location", "win.tm-open-location")
        s2.append("Go to details", "win.tm-goto-details")
        s2.append("Copy command line", "win.tm-copy-cmd")
        s2.append("Properties", "win.tm-properties")
        m.append_section(None, s2)
        self.win.sync_menu_actions()
        self.table.popup(m, x, y)


def _conns(item):
    procs = [item.proc] if item.proc is not None else item.procs
    return sum(p.conns or 0 for p in procs)


class UsersPage(ProcTablePage):
    key = "users"
    title = "Users"

    def __init__(self, win):
        super().__init__(win)
        m = self.app.models
        self.tree = Gtk.TreeListModel.new(
            Gtk.FilterListModel(model=m.users_root, filter=m.user_filter), False, False,
            m.create_user_child)
        self.sorter = Gtk.CustomSorter.new(self._compare)
        self.sort_key, self.sort_desc = "name", False
        sort = Gtk.SortListModel(model=self.tree, sorter=Gtk.TreeListRowSorter.new(self.sorter))
        sel = Gtk.SingleSelection(model=sort, autoselect=False, can_unselect=True)
        mem_total = lambda: max(1, self.app.sys.mem.get("MemTotal", 1))  # noqa: E731
        z = key_sorter(lambda i: 0)
        cols = [
            Col("name", "User", lambda i: i.label, expand=True, kind="name", sort=z),
            Col("status", "Status", lambda i: i.status, width=140, kind="status",
                sort=key_sorter(lambda i: 0)),
            Col("cpu", "CPU", lambda i: fmt_cpu(i.cpu), width=84, kind="num",
                heat=lambda i: i.cpu / 100 * 1.6, sort=key_sorter(lambda i: 0)),
            Col("mem", "Memory", lambda i: fmt_mem(i.mem), width=104, kind="num",
                heat=lambda i: i.mem / mem_total() * 3, sort=key_sorter(lambda i: 0)),
            Col("disk", "Disk", lambda i: fmt_disk(i.disk), width=96, kind="num",
                heat=lambda i: i.disk / (40 * MB), sort=key_sorter(lambda i: 0)),
            Col("net", "Network", net_cell, width=100, kind="num",
                dim=lambda i: net_cell(i) == "—", sort=key_sorter(lambda i: 0)),
        ]
        self.table = Table(sel, cols, on_menu=self._menu, on_activate=self._activate,
                           base_depth=0)
        self.table.cv.get_sorter().connect("changed", self._sort_changed)
        sel.connect("notify::selected-item", lambda *_: self.sync_actions())
        self.box.append(self.table.scroller)
        self.end_btn = tool_button("process-stop-symbolic", "End task", win.end_selected)
        self.actions.append(self.end_btn)
        self.sync_actions()

    def _compare(self, a, b, *_):
        k = self.sort_key
        if k == "name":
            r = cmp(a.label.lower(), b.label.lower())
        elif k == "net":
            r = cmp(_conns(a), _conns(b))
        else:
            r = cmp(getattr(a, k), getattr(b, k))
        return -r if self.sort_desc else r

    def _sort_changed(self, sorter, _c):
        col = sorter.get_primary_sort_column()
        self.sort_key = next((k for k, c in self.table.columns.items() if c == col), "name")
        self.sort_desc = sorter.get_primary_sort_order() == Gtk.SortType.DESCENDING
        self.sorter.changed(Gtk.SorterChange.DIFFERENT)

    def on_tick(self):
        s = self.app.sys
        t = self.table
        t.set_header("name", None, "User", right=False)
        t.set_header("status", None, "Status", right=False)
        t.set_header("cpu", "%d%%" % round(s.cpu), "CPU")
        t.set_header("mem", "%d%%" % round(s.mem_used_fraction() * 100), "Memory")
        t.set_header("disk", "%d%%" % round(s.disk_total_active()), "Disk")
        t.set_header("net", mbps(s.net_total()), "Network")
        if self.sort_key != "name":
            self.sorter.changed(Gtk.SorterChange.DIFFERENT)
        # expand the current user's row once
        if not getattr(self, "_expanded_once", False):
            for i in range(self.tree.get_model().get_n_items()):
                row = self.tree.get_child_row(i)
                if row is not None and row.get_item().key == os.getuid():
                    row.set_expanded(True)
                    self._expanded_once = True
        self.sync_actions()

    def on_show(self, sub=None):
        self.on_tick()

    def on_search(self, text):
        self.app.models.set_query(text)

    def sync_actions(self):
        self.end_btn.set_sensitive(bool(self.selected_procs()))

    def _activate(self, pos):
        row = self.table.selection.get_item(pos)
        if isinstance(row, Gtk.TreeListRow) and row.is_expandable():
            row.set_expanded(not row.get_expanded())

    def _menu(self, cv, x, y):
        item = self.selected()
        if item is None:
            return
        m = Gio.Menu()
        if item.kind == "user":
            m.append("Expand / collapse", "win.tm-expand")
            m.append("End all processes of this user", "win.tm-end")
        else:
            m.append("End task", "win.tm-end")
            m.append("Go to details", "win.tm-goto-details")
            m.append("Open file location", "win.tm-open-location")
            m.append("Copy command line", "win.tm-copy-cmd")
            m.append("Properties", "win.tm-properties")
        self.win.sync_menu_actions()
        self.table.popup(m, x, y)


class DetailsPage(ProcTablePage):
    key = "details"
    title = "Details"

    def __init__(self, win):
        super().__init__(win)
        m = self.app.models
        filt = Gtk.FilterListModel(model=m.details, filter=m.detail_filter)
        self.sort = Gtk.SortListModel(model=filt)
        sel = Gtk.SingleSelection(model=self.sort, autoselect=False, can_unselect=True)
        full = lambda: self.app.settings["full_name"]  # noqa: E731

        def user(i):
            login, name = tp.user_info(i.proc.uid)
            return name if full() and name else login
        mem_total = lambda: max(1, self.app.sys.mem.get("MemTotal", 1))  # noqa: E731
        cols = [
            Col("name", "Name", lambda i: i.proc.name, width=200, kind="name",
                sort=key_sorter(lambda i: i.proc.name.lower())),
            Col("pid", "PID", lambda i: str(i.proc.pid), width=92, kind="num",
                sort=key_sorter(lambda i: i.proc.pid)),
            Col("state", "Status", lambda i: tp.state_name(i.proc.state), width=104,
                sort=key_sorter(lambda i: i.proc.state)),
            Col("user", "User name", user, width=110,
                sort=key_sorter(lambda i: user(i).lower())),
            Col("cpu", "CPU", lambda i: "%.1f" % i.cpu if i.cpu < 9.95 else "%.0f" % i.cpu,
                width=62, kind="num", heat=lambda i: i.cpu / 100 * 1.6,
                sort=key_sorter(lambda i: i.cpu)),
            Col("mem", "Memory (private)", lambda i: "{:,} K".format(i.mem // 1024),
                width=128, kind="num", heat=lambda i: i.mem / mem_total() * 3,
                sort=key_sorter(lambda i: i.mem)),
            Col("threads", "Threads", lambda i: str(i.threads), width=74, kind="num",
                sort=key_sorter(lambda i: i.threads)),
            Col("handles", "Open files", lambda i: "—" if i.proc.nfds is None
                else str(i.proc.nfds), width=86, kind="num",
                sort=key_sorter(lambda i: i.proc.nfds or -1)),
            Col("prio", "Priority", lambda i: "%s (%d)" % (tp.priority_name(i.proc.nice),
                                                           i.proc.nice), width=130,
                sort=key_sorter(lambda i: i.proc.nice)),
            Col("cmd", "Command line", lambda i: i.proc.cmdline, width=420, expand=True,
                sort=key_sorter(lambda i: i.proc.cmdline)),
        ]
        self.table = Table(sel, cols, on_menu=self._menu)
        self.sort.set_sorter(self.table.cv.get_sorter())
        sel.connect("notify::selected-item", lambda *_: self.sync_actions())
        self.box.append(self.table.scroller)
        a = self.actions
        a.append(tool_button("tab-new-symbolic", "Run new task", win.run_new_task))
        self.end_btn = tool_button("process-stop-symbolic", "End task", win.end_selected)
        a.append(self.end_btn)
        self.eff_btn = tool_button("emoji-nature-symbolic", "Efficiency mode",
                                   win.toggle_efficiency)
        a.append(self.eff_btn)
        self.table.cv.sort_by_column(self.table.columns["name"], Gtk.SortType.ASCENDING)
        self.sync_actions()

    def on_tick(self):
        col = self.table.cv.get_sorter().get_primary_sort_column()
        if col is not None and col.get_title() in ("CPU", "Memory (private)", "Threads",
                                                    "Open files"):
            col.get_sorter().changed(Gtk.SorterChange.DIFFERENT)
        self.sync_actions()

    def on_search(self, text):
        self.app.models.set_query(text)

    def sync_actions(self):
        procs = self.selected_procs()
        self.end_btn.set_sensitive(bool(procs))
        self.eff_btn.set_sensitive(bool(procs))

    def select_pid(self, pid):
        for i in range(self.sort.get_n_items()):
            it = self.sort.get_item(i)
            if it.proc.pid == pid:
                self.table.selection.select_item(i, True)
                self.table.cv.scroll_to(i, None, Gtk.ListScrollFlags.FOCUS, None)
                return True
        return False

    def _menu(self, cv, x, y):
        if self.selected() is None:
            return
        m = Gio.Menu()
        s1 = Gio.Menu()
        s1.append("End task", "win.tm-end")
        s1.append("End process tree", "win.tm-end-tree")
        s1.append("Efficiency mode", "win.tm-efficiency")
        m.append_section(None, s1)
        pr = Gio.Menu()
        for name, _n in tp.NICE_LEVELS:
            pr.append(name, "win.tm-priority::" + name)
        m.append_submenu("Set priority", pr)
        s2 = Gio.Menu()
        s2.append("Open file location", "win.tm-open-location")
        s2.append("Copy command line", "win.tm-copy-cmd")
        s2.append("Properties", "win.tm-properties")
        m.append_section(None, s2)
        self.win.sync_menu_actions()
        self.table.popup(m, x, y)


# ------------------------------------------------------------------ window

class TaskWindow(HarborWindow):
    def __init__(self, app):
        super().__init__(app, "Task Manager", 1100, 720, icon=ICON)
        self.app = app
        self.search = Gtk.SearchEntry(placeholder_text="Type a name, publisher, or PID to search",
                                      css_classes=["hb-search"], width_request=360,
                                      valign=Gtk.Align.CENTER)
        self.search.connect("search-changed", self._on_search)
        self.search.connect("stop-search", lambda *_: self.search.set_text(""))
        self.header.set_title_widget(self.search)

        outer = Gtk.Box()
        # navigation column
        self.navcol = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, width_request=230,
                              css_classes=["hb-tm-navcol"], hexpand=False)
        burger = Gtk.Button(icon_name="open-menu-symbolic", css_classes=["hb-tool"],
                            halign=Gtk.Align.START, margin_start=10, margin_bottom=6,
                            tooltip_text="Collapse navigation")
        burger.connect("clicked", lambda *_: self.set_nav_collapsed(not self.nav_collapsed))
        self.navcol.append(burger)
        self.nav = NavList(NAV)
        self.nav.connect("selected", lambda _n, key: self.show_page(key))
        self.navcol.append(self.nav)
        self.navcol.append(Gtk.Box(vexpand=True))
        self.nav2 = NavList([("settings", "Settings", "emblem-system-symbolic")])
        self.nav2.set_margin_bottom(10)
        self.nav2.connect("selected", lambda _n, key: self.show_page(key))
        self.navcol.append(self.nav2)
        outer.append(self.navcol)

        self.surface = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, hexpand=True,
                               css_classes=["hb-surface"])
        self.stack = Gtk.Stack(transition_type=Gtk.StackTransitionType.NONE,
                               hexpand=True, vexpand=True)
        self.surface.append(self.stack)
        outer.append(self.surface)
        self.set_body(outer)

        from . import taskmgr_pages as pg
        from .taskmgr_sweep_page import SweepPage
        self.pages = {}
        for cls in (ProcessesPage, pg.PerformancePage, SweepPage, pg.HistoryPage, pg.StartupPage,
                    UsersPage, DetailsPage, pg.ServicesPage, pg.SettingsPage):
            page = cls(self)
            self.pages[page.key] = page
            self.stack.add_named(page.box, page.key)
        self.current = None
        self._install_actions()
        self._install_shortcuts()
        self.nav_collapsed = False
        self.set_nav_collapsed(bool(app.settings["nav_collapsed"]), save=False)
        start = app.settings["start_page"]
        self.show_page(start if start in PAGE_KEYS else "processes")

    # ---------------------------------------------------------- navigation
    def show_page(self, key):
        """processes, performance[:cpu|memory|swap|diskN|netN|gpuN],
        sweep[:files|suggestions], history, startup, users, details, services,
        settings[:optimization]."""
        base, _, sub = key.partition(":")
        if base not in self.pages:
            base = "processes"
        page = self.pages[base]
        self.current = page
        self.stack.set_visible_child_name(base)
        if base == "settings":
            self.nav.select(None)
            self.nav2.select("settings")
        else:
            self.nav2.select(None)
            self.nav.select(base)
        page.on_search(self.search.get_text())
        page.on_show(sub or None)
        return True

    def set_nav_collapsed(self, on, save=True):
        self.nav_collapsed = on
        self.navcol.set_size_request(52 if on else 230, -1)
        for nav in (self.nav, self.nav2):
            for row in nav.rows.values():
                row.label.set_visible(not on)
                row.set_tooltip_text(row.label.get_label() if on else None)
        if save:
            self.app.settings["nav_collapsed"] = on

    def _on_search(self, entry):
        if self.current is not None:
            self.current.on_search(entry.get_text())

    def on_tick(self):
        if self.current is not None:
            self.current.on_tick()

    # ---------------------------------------------------------- shortcuts
    def _install_shortcuts(self):
        ctl = Gtk.ShortcutController(scope=Gtk.ShortcutScope.MANAGED)
        self.add_controller(ctl)

        def add(trigger, fn):
            ctl.add_shortcut(Gtk.Shortcut.new(Gtk.ShortcutTrigger.parse_string(trigger),
                                              Gtk.CallbackAction.new(fn)))
        add("<Control>f", lambda *_: (self.search.grab_focus(), True)[1])
        add("F5", lambda *_: (self.app.refresh_now(), True)[1])
        add("<Control>r", lambda *_: (self.app.refresh_now(), True)[1])
        add("<Control>n", lambda *_: (self.run_new_task(), True)[1])
        add("<Control>q", lambda *_: (self.close(), True)[1])
        add("<Control>w", lambda *_: (self.close(), True)[1])
        for i, (key, _l, _i) in enumerate(NAV):
            add("<Control>%d" % (i + 1), lambda *_a, k=key: (self.show_page(k), True)[1])

        def delete(*_):
            if self.search.has_focus() or self.search.get_focus_child() is not None:
                return False
            if isinstance(self.get_focus(), Gtk.Editable):
                return False
            if self.current and self.current.key in ("processes", "details", "users"):
                self.end_selected()
                return True
            return False
        add("Delete", delete)
        add("<Shift>Delete", delete)

    # ---------------------------------------------------------- actions
    def _install_actions(self):
        def act(name, fn, param=None):
            a = Gio.SimpleAction.new(name, param)
            a.connect("activate", lambda _a, p: fn(p) if param else fn())
            self.add_action(a)
            return a
        act("tm-expand", self._toggle_expand)
        act("tm-end", self.end_selected)
        act("tm-end-tree", self.end_tree)
        act("tm-efficiency", self.toggle_efficiency)
        self.loc_action = act("tm-open-location", self.open_location)
        act("tm-goto-details", self.goto_details)
        act("tm-properties", self.properties)
        act("tm-copy-cmd", self.copy_cmdline)
        act("tm-priority", lambda p: self.set_priority(p.get_string()),
            GLib.VariantType.new("s"))
        act("expand-all", lambda: self.pages["processes"].expand_all(True))
        act("collapse-all", lambda: self.pages["processes"].expand_all(False))
        s = self.app.settings
        g = Gio.SimpleAction.new_stateful("group-by-type", None,
                                          GLib.Variant.new_boolean(bool(s["group_by_type"])))
        g.connect("change-state", self._group_by_type)
        self.add_action(g)
        hidden = set(s["hidden_columns"])
        for key in ("status", "cpu", "mem", "disk", "net"):
            a = Gio.SimpleAction.new_stateful("col-" + key, None,
                                              GLib.Variant.new_boolean(key not in hidden))
            a.connect("change-state", self._toggle_column, key)
            self.add_action(a)
            if key in hidden:
                self.pages["processes"].table.columns[key].set_visible(False)

    def _group_by_type(self, action, value):
        action.set_state(value)
        self.app.settings["group_by_type"] = value.get_boolean()
        self.app.models.set_group_by_type(value.get_boolean())
        self.pages["processes"].table.base_depth = 1 if value.get_boolean() else 0
        self.pages["processes"].on_tick()

    def _toggle_column(self, action, value, key):
        action.set_state(value)
        self.pages["processes"].table.columns[key].set_visible(value.get_boolean())
        hidden = set(self.app.settings["hidden_columns"])
        if value.get_boolean():
            hidden.discard(key)
        else:
            hidden.add(key)
        self.app.settings["hidden_columns"] = sorted(hidden)

    def sync_menu_actions(self):
        procs = self._procs()
        self.loc_action.set_enabled(any(p.exe for p in procs))

    def _page_table(self):
        page = self.current
        return getattr(page, "table", None) if isinstance(page, ProcTablePage) else None

    def _item(self):
        t = self._page_table()
        return t.selected_item() if t else None

    def _procs(self):
        return self.app.models.procs_of(self._item())

    def _toggle_expand(self):
        t = self._page_table()
        row = t.selected_row() if t else None
        if row is not None and row.is_expandable():
            row.set_expanded(not row.get_expanded())

    def _critical(self, p):
        if p.pid == 1 or p.pid == os.getpid() or p.pid == os.getppid():
            return True
        return p.comm in ("gnome-shell", "systemd", "gdm", "gdm-wayland-ses",
                          "Xwayland") and p.uid == os.getuid()

    def end_selected(self):
        item = self._item()
        procs = self.app.models.procs_of(item)
        if item is not None and item.kind == "kernel":
            self.notify_toast("Kernel threads cannot be ended")
            return
        if procs:
            self.end_procs(procs, item.label if item else "")

    def end_tree(self):
        procs = self._procs()
        if not procs:
            return
        root = procs[0]
        allp = self.app.proc_sampler.procs
        tree = [allp[pid] for pid in tp.descendants(allp, root.pid) if pid in allp]
        # deepest first, the root last
        self.end_procs(list(reversed(tree)) + [root], "%s and its child processes" % root.name)

    def end_procs(self, procs, what):
        procs = [p for p in procs if p.pid > 1 or self._critical(p)]
        me = os.getuid()
        critical = [p for p in procs if self._critical(p)]
        others = [p for p in procs if p.uid != me]

        def go():
            own = [p for p in procs if p.uid == me]
            sent = []
            for p in own:
                if tp.send_signal(p.pid, p.start, signal.SIGTERM):
                    sent.append(p)
            if sent:
                GLib.timeout_add(3000, self._kill_stragglers, sent)
            still = [p for p in others if tp.same_process(p.pid, p.start)]
            if still:
                run_cmd(["pkexec", "kill", "-TERM"] + [str(p.pid) for p in still],
                        lambda ok, _o, err: None if ok else self.notify_toast(
                            "Could not end the process: %s" % (err.strip() or "denied")))
            n = len(sent) + len(still)
            if n:
                self.notify_toast("Ending %s" % what if what else "Ending %d processes" % n)
            elif not still and not sent:
                self.notify_toast("The process already ended")
            GLib.timeout_add(400, lambda: (self.app.refresh_now(), False)[1])

        if critical:
            names = ", ".join(sorted({p.name for p in critical}))
            confirm(self, "End a critical process?",
                    "%s is needed by your session or by Task Manager itself. Ending it can "
                    "close all your apps or log you out." % names, "End process", go)
        elif others:
            confirm(self, "End a process of another user?",
                    "%s runs as another user. Ending it needs administrator rights and "
                    "you will be asked for your password." % what, "End process", go)
        else:
            go()

    def _kill_stragglers(self, procs):
        for p in procs:
            if tp.alive(p.pid, p.start) and not tp.DRY_RUN:
                tp.send_signal(p.pid, p.start, signal.SIGKILL)
            elif tp.DRY_RUN:
                tp.ACTION_LOG.append(["kill-if-alive", "-KILL", str(p.pid)])
        self.app.refresh_now()
        return False

    def _guarded(self, procs, what, go):
        """Run go() directly, or after a confirm when procs include a
        process the session or Task Manager depends on."""
        critical = [p for p in procs if self._critical(p)]
        if not critical:
            go()
            return
        names = ", ".join(sorted({p.name for p in critical}))
        confirm(self, "Change a critical process?",
                "%s is needed by your session or by Task Manager itself. Changing its "
                "priority can make the whole desktop slow." % names, what, go,
                destructive=False)

    def toggle_efficiency(self):
        procs = self._procs()
        if procs:
            self._guarded(procs, "Change priority", lambda: self._efficiency(procs))

    def _efficiency(self, procs):
        me = os.getuid()
        if all(p.efficiency for p in procs):
            # back to normal priority: lowering nice needs root
            live = [p for p in procs if tp.same_process(p.pid, p.start)]
            if live:
                run_cmd(["pkexec", "renice", "0", "-p"] + [str(p.pid) for p in live],
                        lambda ok, _o, _e: (self.app.refresh_now(), self.notify_toast(
                            "Efficiency mode turned off" if ok else
                            "Could not change the priority")))
            return
        need_root = []
        for p in procs:
            if p.uid == me:
                r = tp.set_nice(p.pid, p.start, 19)
                if r == "need-root":
                    need_root.append(p)
            else:
                need_root.append(p)
        if need_root:
            run_cmd(["pkexec", "renice", "19", "-p"] + [str(p.pid) for p in need_root],
                    lambda ok, _o, _e: self.app.refresh_now())
        self.notify_toast("Efficiency mode on: lowest CPU priority")
        self.app.refresh_now()

    def set_priority(self, name):
        procs = self._procs()
        if procs:
            self._guarded(procs, "Change priority", lambda: self._priority(procs, name))

    def _priority(self, procs, name):
        nice = tp.nice_for(name)
        for p in procs:
            r = tp.set_nice(p.pid, p.start, nice) if p.uid == os.getuid() else "need-root"
            if r == "need-root" and tp.same_process(p.pid, p.start):
                run_cmd(["pkexec", "renice", str(nice), "-p", str(p.pid)],
                        lambda ok, _o, _e: self.app.refresh_now())
            elif r == "gone":
                self.notify_toast("The process already ended")
        self.app.refresh_now()

    def open_location(self):
        procs = [p for p in self._procs() if p.exe]
        if not procs:
            self.notify_toast("The file location is not readable for this process")
            return
        d = os.path.dirname(procs[0].exe.replace(" (deleted)", ""))
        if have("harbor-files"):
            spawn(["harbor-files", d])
        else:
            spawn(["xdg-open", d])

    def goto_details(self):
        procs = self._procs()
        if not procs:
            return
        self.show_page("details")
        self.pages["details"].select_pid(procs[0].pid)

    def copy_cmdline(self):
        procs = self._procs()
        if procs:
            copy_text(self, "\n".join(p.cmdline for p in procs))
            self.notify_toast("Command line copied")

    def properties(self):
        item = self._item()
        procs = self.app.models.procs_of(item)
        if not procs:
            return
        p = procs[0]
        login, full = tp.user_info(p.uid)
        boot = self.app.sys.extra.get("btime", 0)
        started = GLib.DateTime.new_from_unix_local(int(boot + p.start / tp.CLK_TCK))
        rows = []
        if item.kind in ("app", "unit"):
            rows.append(("Group", "%s (%d processes)" % (item.name, len(procs))))
        rows += [("Name", p.name), ("PID", p.pid), ("Parent PID", p.ppid),
                 ("User", "%s (%s)" % (login, full) if full != login else login),
                 ("Status", tp.state_name(p.state)),
                 ("Executable", p.exe or "Not readable (process of another user)"),
                 ("Command line", p.cmdline),
                 ("Started", started.format("%x %X") if started else "?"),
                 ("Threads", p.threads),
                 ("Priority", "%s (nice %d)" % (tp.priority_name(p.nice), p.nice)),
                 ("Memory (private)", fmt_mem(p.mem)),
                 ("CPU time", ts.format_cpu_time(p.ticks / tp.CLK_TCK)),
                 ("Open files", "—" if p.nfds is None else p.nfds),
                 ("Network connections", "—" if p.conns is None else p.conns),
                 ("Control group", p.cgroup or "—")]
        info_dialog(self, "Properties of %s" % p.name, rows)

    def run_new_task(self):
        dlg = Adw.AlertDialog(heading="Create new task",
                              body="Type the name of a program, folder, document or "
                                   "internet resource, and it will open for you.")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        row = Gtk.Box(spacing=8)
        entry = Gtk.Entry(placeholder_text="Open", activates_default=True, hexpand=True)
        row.append(entry)
        browse = Gtk.Button(label="Browse\u2026")

        def picked(fd, res):
            try:
                f = fd.open_finish(res)
            except GLib.Error:
                return
            if f is not None and f.get_path():
                entry.set_text(GLib.shell_quote(f.get_path()))

        def do_browse(*_):
            fd = Gtk.FileDialog(title="Browse", modal=True)
            fd.set_initial_folder(Gio.File.new_for_path("/usr/bin"))
            fd.open(self, None, picked)
        browse.connect("clicked", do_browse)
        row.append(browse)
        box.append(row)
        admin = Gtk.CheckButton(label="Create this task with administrative privileges")
        box.append(admin)
        dlg.set_extra_child(box)
        dlg.add_response("cancel", "Cancel")
        dlg.add_response("ok", "OK")
        dlg.set_response_appearance("ok", Adw.ResponseAppearance.SUGGESTED)
        dlg.set_default_response("ok")
        dlg.set_close_response("cancel")

        def response(_d, r):
            if r != "ok":
                return
            text = entry.get_text().strip()
            if not text:
                return
            try:
                ok, argv = GLib.shell_parse_argv(text)
            except GLib.Error as e:
                self.notify_toast("Cannot run: %s" % e.message)
                return
            if not ok or not argv:
                return
            if "://" in argv[0] or (len(argv) == 1 and os.path.exists(
                    os.path.expanduser(argv[0])) and not os.access(
                    os.path.expanduser(argv[0]), os.X_OK)):
                argv = ["xdg-open", os.path.expanduser(argv[0])]
            elif not have(argv[0]) and not os.access(os.path.expanduser(argv[0]), os.X_OK):
                self.notify_toast("Cannot find “%s”" % argv[0])
                return
            if admin.get_active():
                argv = ["pkexec"] + argv
            if not spawn(argv):
                self.notify_toast("Could not start “%s”" % argv[0])
        dlg.connect("response", response)
        dlg.present(self)
        entry.grab_focus()
        return dlg


# ------------------------------------------------------------------ application

class App(Adw.Application):
    def __init__(self):
        super().__init__(application_id=APP_ID,
                         flags=Gio.ApplicationFlags.HANDLES_COMMAND_LINE)
        self.add_main_option("page", 0, GLib.OptionFlags.NONE, GLib.OptionArg.STRING,
                             "Page to open (processes, performance, performance:cpu, "
                             "performance:swap, sweep, history, startup, users, details, "
                             "services, settings, settings:optimization)", "NAME")
        self.settings = ts.Settings(config_path("taskmgr.json"))
        self.history = ts.AppHistory(data_path("app-history.json"))
        self.proc_sampler = tp.ProcSampler()
        self.sys = tp.SystemSampler()
        self.models = None
        self.timer = None
        self.last_save = 0
        self.win = None
        self.last_mem_warning = -ms.NOTIFY_EVERY

    def do_startup(self):
        Adw.Application.do_startup(self)
        apply_theme(CSS)
        here = os.path.dirname(os.path.realpath(__file__))
        for d in (os.path.join(here, "..", "..", "data", "icons"),
                  "/usr/share/harbor/icons"):
            if os.path.isdir(d):
                from gi.repository import Gdk
                Gtk.IconTheme.get_for_display(Gdk.Display.get_default()).add_search_path(d)
        self.models = ProcModels(AppLookup())
        self.models.full_names = bool(self.settings["full_name"])
        self.models.group_by_type = bool(self.settings["group_by_type"])
        self._sample()
        self.set_accels_for_action("app.quit", ["<Control>q"])
        show = Gio.SimpleAction.new("show-page", GLib.VariantType.new("s"))
        show.connect("activate", self._show_page_action)
        self.add_action(show)

    def do_command_line(self, cmdline):
        opts = cmdline.get_options_dict()
        page = opts.lookup_value("page", GLib.VariantType.new("s"))
        self.activate()
        if page is not None and self.win is not None:
            self.win.show_page(page.get_string())
        return 0

    def do_activate(self):
        if self.win is None:
            self.win = TaskWindow(self)
            self.win.connect("close-request", self._on_close)
            self.set_interval(self.settings.interval_ms())
            if self._poll_nvidia():
                GLib.timeout_add_seconds(2, self._poll_nvidia)
            self.models.update(self.proc_sampler.procs, self.sys.mem.get("MemTotal"))
        self.win.present()

    def _on_close(self, *_):
        self._save_history(force=True)
        return False

    def set_interval(self, ms):
        if self.timer:
            GLib.source_remove(self.timer)
            self.timer = None
        if ms > 0:
            self.timer = GLib.timeout_add(ms, self._tick)

    def _sample(self):
        procs = self.proc_sampler.sample()
        self.sys.sample(procs, self.proc_sampler.my_uid)
        return procs

    def _poll_nvidia(self):
        if any(g.driver == "nvidia" for g in self.sys.gpus) and have("nvidia-smi"):
            run_cmd(tp.NVIDIA_QUERY, lambda ok, out, _e: setattr(
                self.sys, "nvidia", tp.parse_nvidia_smi(out) if ok else []), mutating=False)
            return True
        return False

    def refresh_now(self):
        self._tick()

    def _show_page_action(self, _a, param):
        self.activate()
        if self.win is not None:
            self.win.show_page(param.get_string())

    def _check_memory(self):
        """High-memory warning (Settings > Notifications), rate-limited."""
        m = self.sys.mem
        total = m.get("MemTotal", 0)
        if not total:
            return
        pct = 100.0 * (total - m.get("MemAvailable", 0)) / total
        now = GLib.get_monotonic_time() / 1e6
        if not ms.should_notify(pct, self.settings["notify_memory"],
                                self.settings["notify_at"], self.last_mem_warning, now):
            return
        self.last_mem_warning = now
        import threading

        def work():
            try:
                groups = ms.build_groups(ms.scan_procs(), ms.self_and_ancestors())
            except OSError:
                groups = []
            GLib.idle_add(send, groups)

        def send(groups):
            title, body = ms.notification_text(pct, groups)
            n = Gio.Notification.new(title)
            n.set_body(body)
            n.set_icon(Gio.ThemedIcon.new(ICON))
            n.set_priority(Gio.NotificationPriority.HIGH)
            n.set_default_action_and_target("app.show-page", GLib.Variant("s", "sweep"))
            n.add_button_with_target("Open Memory Sweep", "app.show-page",
                                     GLib.Variant("s", "sweep"))
            self.send_notification("memory-high", n)
            return False
        threading.Thread(target=work, daemon=True).start()

    def _tick(self):
        procs = self._sample()
        self.models.update(procs, self.sys.mem.get("MemTotal"))
        self._track_history()
        self._check_memory()
        if self.win is not None:
            self.win.on_tick()
        return True

    def _track_history(self):
        for (g, key), c in self.models.containers.items():
            if g != "apps":
                continue
            cpu = sum(p.cpu_seconds for p in c.procs)
            disk = sum(p.disk_delta for p in c.procs)
            icon = c.icon.to_string() if c.icon is not None else ""
            self.history.add(key, c.name, cpu, disk, icon or "")
        self._save_history()

    def _save_history(self, force=False):
        now = GLib.get_monotonic_time() / 1e6
        if self.history.dirty and (force or now - self.last_save > 30):
            self.last_save = now
            try:
                self.history.save()
            except OSError:
                pass


def main(argv=None):
    argv = list(argv if argv is not None else sys.argv)
    if len(argv) > 1 and argv[1] == "sweep-report":
        return ms.report("--json" in argv[2:])
    app = App()
    return app.run(argv)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
