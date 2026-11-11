"""List models for Harbor Task Manager.

One Item per process is shared by the Processes tree, the Details list and
the Users tree. Every refresh updates the existing items in place and only
inserts or removes the rows of processes that started or ended, so
selection, expansion and scroll position survive.
"""

import os
import re

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gio, GLib, GObject, Gtk  # noqa: E402

from . import taskmgr_proc as tp  # noqa: E402

MB = 1024.0 * 1024.0


def fmt_cpu(v):
    if v < 0.05:
        return "0%"
    if v < 10:
        return "%.1f%%" % v
    return "%.0f%%" % v


def fmt_mem(b):
    return "{:,.1f} MB".format(b / MB)


def fmt_disk(r):
    if r < 0.05 * MB:
        return "0 MB/s"
    return "{:,.1f} MB/s".format(r / MB)


class Item(GObject.Object):
    """A row: group, app, unit (service with several processes), user,
    kernel (all kernel threads) or proc. Emits "changed" when a shown
    value changed."""

    __gtype_name__ = "HarborTmItem"
    __gsignals__ = {"changed": (GObject.SignalFlags.RUN_FIRST, None, ())}

    def __init__(self, kind, key, name="", order=0):
        super().__init__()
        self.kind = kind
        self.key = key
        self.name = name
        self.label = name
        self.icon = None  # Gio.Icon or None
        self.icon_name = "application-x-executable-symbolic"
        self.order = order
        self.count = 0
        self.cpu = 0.0
        self.mem = 0
        self.disk = 0.0
        self.threads = 0
        self.status = ""
        self.proc = None
        self.procs = []
        self.match = True
        self.app_id = None
        self.children = Gio.ListStore(item_type=Item) if kind != "proc" and kind != "kernel" \
            else None

    def set_values(self, **kw):
        changed = False
        for k, v in kw.items():
            if getattr(self, k) != v:
                setattr(self, k, v)
                changed = True
        if changed:
            self.emit("changed")
        return changed

    # --- text helpers used by the cells
    @property
    def pid(self):
        return self.proc.pid if self.proc else None


def proc_status(procs):
    if procs and all(p.suspended for p in procs):
        return "Suspended"
    if procs and all(p.efficiency for p in procs):
        return "Efficiency mode"
    return ""


def sync_store(store, items):
    """Make store contain exactly items (order kept for existing ones)."""
    want = {id(i): i for i in items}
    have = set()
    n = store.get_n_items()
    for pos in range(n - 1, -1, -1):
        it = store.get_item(pos)
        if id(it) not in want:
            store.remove(pos)
        else:
            have.add(id(it))
    new = [i for i in items if id(i) not in have]
    if new:
        store.splice(store.get_n_items(), 0, new)
    return bool(new) or len(have) != n


def proc_matches(p, q):
    if not q:
        return True
    if q.isdigit() and str(p.pid).startswith(q):
        return True
    return q in p.name.lower() or q in p.cmdline.lower()


# Exec programs that only launch something else: never used to match an app.
LAUNCHER_EXECS = {"env", "sh", "bash", "dash", "flatpak", "snap", "gjs", "java", "node",
                  "electron", "python", "python3", "perl", "ruby", "wine", "gtk-launch",
                  "gio", "xdg-open", "pkexec", "sudo", "nice", "ionice", "bwrap", "toolbox",
                  "distrobox", "appimage-run", "steam"}


def _id_of(desktop_id):
    return desktop_id[:-8] if desktop_id.endswith(".desktop") else desktop_id


def _exec_base(path):
    base = os.path.basename(path or "").lower()
    if not base or base in LAUNCHER_EXECS or re.fullmatch(r"python[\d.]*", base):
        return ""
    return base


def installed_apps():
    """Every installed .desktop entry as {"id", "name", "icon", "visible",
    "wmclass", "exec"}."""
    out = []
    for info in Gio.AppInfo.get_all():
        did = info.get_id() or ""
        if not did:
            continue
        get = getattr(info, "get_startup_wm_class", None)
        out.append({"id": _id_of(did), "name": info.get_display_name() or info.get_name(),
                    "icon": info.get_icon(), "visible": not getattr(
                        info, "get_nodisplay", lambda: False)(),
                    "wmclass": (get() if get else None) or "",
                    "exec": info.get_executable() or ""})
    return out


class AppLookup:
    """App id -> (name, Gio.Icon, visible) from the installed .desktop files.

    resolve() also folds app units without a .desktop file of their own (an
    autostart helper like app-gnome-io.github.foo.bar-123.scope next to the
    app's own app-bar-123.scope) into the visible app whose desktop id, last
    id component, StartupWMClass or Exec basename matches case-insensitively.
    Units that match no visible app are background processes, like on
    Windows. entries: installed_apps() rows (tests pass their own)."""

    def __init__(self, entries=None):
        self.cache = {}
        self.resolved = {}
        self._entries = entries
        self._by_id = None
        self._alias = None

    def _index(self):
        if self._by_id is None:
            entries = self._entries if self._entries is not None else installed_apps()
            self._by_id, self._alias = {}, {}
            for e in entries:
                self._by_id.setdefault(e["id"], e)
                self._by_id.setdefault(e["id"].lower(), e)
            # visible apps only; exact ids first, then weaker aliases
            for weak in (False, True):
                for e in entries:
                    if not e.get("visible", True):
                        continue
                    keys = ([e["id"].lower()] if not weak else
                            [e["id"].rsplit(".", 1)[-1].lower(), (e.get("wmclass") or "").lower(),
                             _exec_base(e.get("exec"))])
                    for k in keys:
                        if k:
                            self._alias.setdefault(k, e)
        return self._by_id

    def _entry(self, cand):
        """The .desktop entry of a desktop id candidate ("foo.desktop")."""
        by_id = self._index()
        key = _id_of(cand)
        e = by_id.get(key) or by_id.get(key.lower())
        if e is not None or self._entries is not None:
            return e
        try:  # installed after the index was built
            info = Gio.DesktopAppInfo.new(cand)
        except (TypeError, GLib.Error):
            info = None
        if info is None:
            return None
        e = {"id": key, "name": info.get_display_name() or info.get_name(),
             "icon": info.get_icon(), "visible": not info.get_nodisplay(),
             "wmclass": info.get_startup_wm_class() or "", "exec": info.get_executable() or ""}
        by_id[key] = e
        return e

    def __call__(self, app):
        key = app["id"]
        if key in self.cache:
            return self.cache[key]
        name, icon, visible = None, None, True
        for cand in app.get("desktop", [key + ".desktop"]):
            e = self._entry(cand)
            if e is not None:
                name, icon, visible = e["name"], e["icon"], e.get("visible", True)
                break
        else:
            visible = app.get("kind") == "scope"
        res = (name or key.rsplit(".", 1)[-1], icon, visible)
        self.cache[key] = res
        return res

    def resolve(self, app, exes=()):
        """(container key, name, Gio.Icon) of the visible app a unit belongs
        to, or None when it is a background unit. exes: executables of the
        unit's processes (a str or a list), the last resort for matching."""
        if isinstance(exes, str):
            exes = [exes]
        bases = tuple(sorted({b for b in map(_exec_base, exes) if b}))
        ck = (app["id"], bases)
        if ck in self.resolved:
            return self.resolved[ck]
        res = None
        own = None
        for cand in app.get("desktop", [app["id"] + ".desktop"]):
            own = self._entry(cand)
            if own is not None:
                break
        if own is not None and own.get("visible", True):
            res = (own["id"], own["name"], own["icon"])
        else:
            self._index()
            low = app["id"].lower()
            for k in (low, low.rsplit(".", 1)[-1]) + bases:
                e = self._alias.get(k) if k else None
                if e is not None:
                    res = (e["id"], e["name"], e["icon"])
                    break
        self.resolved[ck] = res
        return res


class ProcModels:
    """Builds and updates the Processes tree, Details list and Users tree."""

    GROUPS = [("apps", "Apps"), ("background", "Background processes"),
              ("system", "System processes")]

    def __init__(self, lookup=None, my_uid=None):
        import os
        self.lookup = lookup or AppLookup()
        self.my_uid = os.getuid() if my_uid is None else my_uid
        self.root = Gio.ListStore(item_type=Item)
        self.groups = {}
        for i, (k, label) in enumerate(self.GROUPS):
            g = Item("group", k, label, order=i)
            self.groups[k] = g
        self.group_by_type = True
        self.items = {}  # (pid, start) -> Item
        self.containers = {}  # (group, key) -> Item
        self.kernel = Item("kernel", "kernel", "System (kernel)")
        self.kernel.icon_name = "computer-symbolic"
        self.details = Gio.ListStore(item_type=Item)
        self.users_root = Gio.ListStore(item_type=Item)
        self.users = {}
        self.query = ""
        self.full_names = False
        self.tree_filter = Gtk.CustomFilter.new(self._tree_filter_func)
        self.user_filter = Gtk.CustomFilter.new(self._user_filter_func)
        self.detail_filter = Gtk.CustomFilter.new(self._detail_filter_func)
        self.mem_total = 1
        self.totals = {"cpu": 0.0, "mem": 0, "disk": 0.0}
        self._nonempty = {}
        self._app_info = {}  # container key -> (key, name, icon) from lookup.resolve
        self._unit_exes = {}

    # ---------------------------------------------------------- filters
    def _tree_filter_func(self, item, *_):
        if item.kind == "group":
            return item.children.get_n_items() > 0 and (not self.query or item.match)
        return item.match

    def _user_filter_func(self, item, *_):
        if item.kind == "user":
            return not self.query or any(proc_matches(c, self.query) for c in item.procs)
        return item.proc is None or proc_matches(item.proc, self.query)

    def _detail_filter_func(self, item, *_):
        return proc_matches(item.proc, self.query)

    def set_query(self, text):
        self.query = (text or "").strip().lower()
        self._compute_matches()
        for f in (self.tree_filter, self.user_filter, self.detail_filter):
            f.changed(Gtk.FilterChange.DIFFERENT)

    def _compute_matches(self):
        q = self.query
        for item in self.items.values():
            item.match = proc_matches(item.proc, q)
        for c in self.containers.values():
            own = not q or q in c.name.lower()
            if own and q:
                for ch in c.procs:
                    self.items[(ch.pid, ch.start)].match = True
            c.match = own or any(self.items[(ch.pid, ch.start)].match for ch in c.procs)
        self.kernel.match = not q or q in "system (kernel)"
        for g in self.groups.values():
            tops = _store_items(g.children)
            n = sum(1 for it in tops if it.match) if q else len(tops)
            g.match = n > 0
            if self.group_by_type:
                g.set_values(label="%s (%d)" % (g.name, n))

    @staticmethod
    def _top_match(item):
        return item.match

    def create_tree_child(self, item, *_):
        if item.children is None:
            return None
        return Gtk.FilterListModel(model=item.children, filter=self.tree_filter)

    def create_user_child(self, item, *_):
        if item.kind != "user":
            return None
        return Gtk.FilterListModel(model=item.children, filter=self.user_filter)

    # ---------------------------------------------------------- update
    def _proc_item(self, p):
        key = (p.pid, p.start)
        it = self.items.get(key)
        if it is None:
            it = Item("proc", key, p.name)
            it.proc = p
            self.items[key] = it
        it.proc = p
        return it

    def _place(self, p):
        """(group, container key or None) for a process."""
        g, key = tp.classify(p, self.my_uid)
        if g == "apps":
            resolve = getattr(self.lookup, "resolve", None)
            if resolve is not None:
                found = resolve(p.app, self._unit_exes.get(p.unit, ()))
                if found is None:
                    return "background", p.unit
                self._app_info[found[0]] = found
                return "apps", found[0]
            name, icon, visible = self.lookup(p.app)
            if not visible and p.app.get("kind") == "service":
                return "background", p.unit
            return "apps", key
        return g, key

    def update(self, procs, mem_total=None):
        if mem_total:
            self.mem_total = mem_total
        live = set()
        buckets = {"apps": {}, "background": {}, "system": {}}
        kernel_cpu = 0.0
        kernel_threads = 0
        kernel_n = 0
        all_items = []
        # a unit is placed as a whole: every executable in it may name the app
        self._unit_exes = {}
        for p in procs.values():
            if p.app is not None and p.exe and not p.kernel:
                self._unit_exes.setdefault(p.unit, []).append(p.exe)
        for p in procs.values():
            if p.kernel:
                kernel_cpu += p.cpu
                kernel_threads += p.threads
                kernel_n += 1
                continue
            it = self._proc_item(p)
            live.add(it.key)
            all_items.append(it)
            it.set_values(name=p.name, label=p.name, cpu=round(p.cpu, 1),
                          mem=p.mem, disk=round(p.disk / 1e4) * 1e4,
                          threads=p.threads, status=proc_status([p]), count=0)
            g, key = self._place(p)
            buckets[g].setdefault(key, []).append(it)
        for key in [k for k in self.items if k not in live]:
            del self.items[key]
        # containers
        used = set()
        tops = {"apps": [], "background": [], "system": []}
        for g, units in buckets.items():
            for key, members in units.items():
                if g != "apps" and len(members) == 1:
                    tops[g].append(members[0])
                    continue
                ckey = (g, key)
                used.add(ckey)
                c = self.containers.get(ckey)
                if c is None:
                    c = Item("app" if g == "apps" else "unit", ckey)
                    self.containers[ckey] = c
                members.sort(key=lambda i: i.proc.pid)
                c.procs = [m.proc for m in members]
                if g == "apps":
                    info = self._app_info.get(key)
                    if info is not None:
                        name, icon = info[1], info[2]
                    else:
                        name, icon, _v = self.lookup(members[0].proc.app)
                    c.icon = icon
                    c.app_id = key
                else:
                    pids = {m.proc.pid for m in members}
                    leader = next((m for m in members if m.proc.ppid not in pids), members[0])
                    name = leader.proc.name
                    c.icon = None
                n = len(members)
                c.set_values(name=name, label="%s (%d)" % (name, n) if n > 1 else name,
                             count=n, cpu=round(sum(m.cpu for m in members), 1),
                             mem=sum(m.mem for m in members),
                             disk=sum(m.disk for m in members),
                             threads=sum(m.threads for m in members),
                             status=proc_status(c.procs))
                sync_store(c.children, members)
                tops[g].append(c)
        for ckey in [k for k in self.containers if k not in used]:
            c = self.containers.pop(ckey)
            sync_store(c.children, [])
        if kernel_n:
            self.kernel.set_values(cpu=round(kernel_cpu, 1), threads=kernel_threads,
                                   count=kernel_n)
            tops["system"].append(self.kernel)
        refilter = False
        for g, items in tops.items():
            grp = self.groups[g]
            changed = sync_store(grp.children, items)
            n = len(items)
            if self.group_by_type:
                grp.set_values(label="%s (%d)" % (grp.name, n), count=n)
            nonempty = n > 0
            if self._nonempty.get(g) != nonempty:
                self._nonempty[g] = nonempty
                refilter = True
            if changed and self.query:
                refilter = True
        if self.group_by_type:
            sync_store(self.root, [self.groups[g] for g, _ in self.GROUPS])
        else:
            for grp in self.groups.values():
                pass
            sync_store(self.root, tops["apps"] + tops["background"] + tops["system"])
        sync_store(self.details, all_items)
        self._update_users(all_items)
        if self.query:
            self._compute_matches()
            refilter = True
        if refilter:
            self.tree_filter.changed(Gtk.FilterChange.DIFFERENT)
            if self.query:
                self.user_filter.changed(Gtk.FilterChange.DIFFERENT)
                self.detail_filter.changed(Gtk.FilterChange.DIFFERENT)

    def set_group_by_type(self, on):
        if on == self.group_by_type:
            return
        self.group_by_type = on
        # the same items cannot sit in a group store and the root at once
        # in a tree, so clear the old layout first
        sync_store(self.root, [])
        if on:
            sync_store(self.root, [self.groups[g] for g, _ in self.GROUPS])
        else:
            items = []
            for g, _ in self.GROUPS:
                items += _store_items(self.groups[g].children)
            sync_store(self.root, items)

    def _update_users(self, items):
        per = {}
        for it in items:
            per.setdefault(it.proc.uid, []).append(it)
        logged = {self.my_uid}
        for it in items:
            u = tp.uid_of_user_slice(it.proc.cgroup)
            if u is not None:
                logged.add(u)
        rows = []
        for uid in sorted(logged):
            members = per.get(uid, [])
            if not members:
                continue
            u = self.users.get(uid)
            if u is None:
                u = Item("user", uid)
                u.icon_name = "avatar-default-symbolic"
                self.users[uid] = u
            login, full = tp.user_info(uid)
            name = ("%s (%s)" % (full, login) if full and full != login else login) \
                if self.full_names else login
            u.procs = [m.proc for m in members]
            u.set_values(name=name, label="%s (%d)" % (name, len(members)),
                         count=len(members), cpu=round(sum(m.cpu for m in members), 1),
                         mem=sum(m.mem for m in members),
                         disk=sum(m.disk for m in members), status="")
            sync_store(u.children, members)
            rows.append(u)
        for uid in [k for k in self.users if k not in logged or not per.get(k)]:
            sync_store(self.users.pop(uid).children, [])
        sync_store(self.users_root, rows)

    # ---------------------------------------------------------- helpers
    def procs_of(self, item):
        """Processes an action on item applies to."""
        if item is None:
            return []
        if item.kind == "proc":
            return [item.proc]
        if item.kind in ("app", "unit", "user"):
            return list(item.procs)
        return []


def _store_items(store):
    return [store.get_item(i) for i in range(store.get_n_items())]
